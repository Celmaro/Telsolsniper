#!/usr/bin/env node
/**
 * Capture recent channel messages with every link surface, to JSONL.
 *
 * For each configured channel: fetch up to N recent messages (default 100)
 * and record, per message:
 *   - text (verbatim) and date
 *   - URL entities (MessageEntityUrl) and hidden links (MessageEntityTextUrl)
 *   - inline-keyboard URLs, button labels and callback data
 *   - resolved mint candidates per surface (so we can see which surfaces
 *     actually carry the contract address)
 *
 * Output: data/captures/<channel>.jsonl (append, deduped by message id)
 *         data/captures/summary-<timestamp>.json
 *
 * Run: npm run capture -- --limit 100
 */
import { readFile, writeFile, mkdir, appendFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { TelegramClient } from "teleproto";
import { StringSession } from "teleproto/sessions";
import { loadConfig } from "../src/config.ts";

const BASE58_MINT = /\b[1-9A-HJ-NP-Za-km-z]{32,44}\b/g;
/** Hosts that put the mint in a path segment (observed on these channels). */
const MINT_URL_PATTERNS = [
  /soulterminal\.ai\/token\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  /solscan\.io\/token\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  /pump\.fun\/coin\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  /bags\.fm\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  /[?&]start=(?:ttb_|fakevol_|tts_)?([1-9A-HJ-NP-Za-km-z]{32,44})/i
];

function mintFromUrl(url) {
  if (!url) return null;
  for (const pattern of MINT_URL_PATTERNS) {
    const match = url.match(pattern);
    if (match) return match[1];
  }
  return null;
}

function mintFromText(text) {
  if (!text) return [];
  const found = new Set();
  for (const match of text.matchAll(BASE58_MINT)) {
    // pump mints end in "pump"; other 32-44 base58 strings may be wallets
    found.add(match[0]);
  }
  return [...found];
}

/** Button label + URL/callback from a raw TL inline button. */
function describeButton(button) {
  const type = button?.type ?? {};
  const className = type?.className ?? type?.constructor?.name ?? "unknown";
  const unit = {
    text: button?.text ?? null,
    kind: className,
    url: type?.url ?? null,
    data: null,
    dataText: null
  };
  if (type?.data != null) {
    const buffer = Buffer.from(type.data);
    unit.data = buffer.toString("base64");
    unit.dataText = buffer.toString("utf8").replace(/[^\x20-\x7e]/g, ".");
  }
  return unit;
}

function flattenRows(replyMarkup) {
  const rows = replyMarkup?.rows ?? [];
  return rows.map((row, rowIndex) =>
    (row?.buttons ?? []).map((button, colIndex) => ({
      row: rowIndex,
      col: colIndex,
      ...describeButton(button)
    }))
  );
}

function entitySummary(entity) {
  const className = entity?.className ?? entity?.constructor?.name ?? "unknown";
  const unit = {
    type: className,
    offset: entity?.offset ?? null,
    length: entity?.length ?? null
  };
  if (entity?.url) unit.url = entity.url;
  return unit;
}

async function loadSession(path) {
  try {
    return (await readFile(path, "utf8")).trim();
  } catch (error) {
    if (error.code === "ENOENT") return "";
    throw error;
  }
}

async function readExistingIds(path) {
  if (!existsSync(path)) return new Set();
  const content = await readFile(path, "utf8");
  const ids = new Set();
  for (const line of content.split("\n")) {
    if (!line.trim()) continue;
    try {
      const record = JSON.parse(line);
      if (record.messageId != null) ids.add(record.messageId);
    } catch {
      // ignore malformed lines
    }
  }
  return ids;
}

async function main() {
  const limitIndex = process.argv.indexOf("--limit");
  const limit = limitIndex >= 0 ? Number(process.argv[limitIndex + 1]) : 100;

  const config = loadConfig();
  const sessionValue = await loadSession(config.sessionFile);
  if (!sessionValue) {
    throw new Error("No Telegram session; run the listener login first");
  }

  const client = new TelegramClient(
    new StringSession(sessionValue),
    config.apiId,
    config.apiHash,
    { connectionRetries: 5 }
  );
  await client.connect();

  const captureDir = join(process.cwd(), "data", "captures");
  await mkdir(captureDir, { recursive: true });

  const summary = { startedAt: new Date().toISOString(), limit, channels: {} };
  let grandTotal = 0;

  for (const channel of config.channels) {
    const path = join(captureDir, `${channel}.jsonl`);
    const known = await readExistingIds(path);
    let messages = [];
    try {
      messages = await client.getMessages(channel, { limit });
    } catch (error) {
      summary.channels[channel] = { error: String(error?.message ?? error) };
      console.error(`[${channel}] fetch failed: ${error?.message ?? error}`);
      continue;
    }

    const stats = {
      fetched: messages.length,
      newRecords: 0,
      withText: 0,
      withUrlEntity: 0,
      withHiddenUrl: 0,
      withButtons: 0,
      withCallbackData: 0,
      mintInText: 0,
      mintInUrl: 0,
      mintInCallback: 0,
      noMintAnywhere: 0,
      hosts: {}
    };

    const lines = [];
    for (const raw of messages) {
      if (raw?.id == null) continue;
      if (known.has(raw.id)) continue;

      const replyMarkup = raw.replyMarkup ?? raw.reply_markup ?? null;
      const rows = flattenRows(replyMarkup);
      const buttons = rows.flat();
      const text = raw.message ?? "";
      const entities = (raw.entities ?? []).map(entitySummary);

      const entityUrls = entities.filter((e) => e.type === "MessageEntityUrl" && e.url).map((e) => e.url);
      // Entities don't carry the URL for MessageEntityUrl; the text slice does.
      for (const entity of entities) {
        if (entity.type === "MessageEntityUrl" && entity.offset != null) {
          entityUrls.push(text.slice(entity.offset, entity.offset + entity.length));
        }
      }
      const hiddenUrls = entities
        .filter((e) => e.type === "MessageEntityTextUrl" && e.url)
        .map((e) => e.url);
      const buttonUrls = buttons.filter((b) => b.url).map((b) => b.url);
      const callbackData = buttons.filter((b) => b.dataText).map((b) => b.dataText);

      const allUrls = [...new Set([...entityUrls, ...hiddenUrls, ...buttonUrls])];
      const textMints = mintFromText(text);
      const urlMints = allUrls.map(mintFromUrl).filter(Boolean);
      const callbackMints = callbackData
        .flatMap((data) => mintFromText(data))
        .filter((mint) => mint.length >= 32);
      const callbackUrlMints = callbackData.map(mintFromUrl).filter(Boolean);

      const mints = [...new Set([...urlMints, ...callbackMints, ...callbackUrlMints])];

      if (text) stats.withText += 1;
      if (entityUrls.length) stats.withUrlEntity += 1;
      if (hiddenUrls.length) stats.withHiddenUrl += 1;
      if (buttons.length) stats.withButtons += 1;
      if (callbackData.length) stats.withCallbackData += 1;
      if (textMints.length) stats.mintInText += 1;
      if (urlMints.length) stats.mintInUrl += 1;
      if (callbackMints.length || callbackUrlMints.length) stats.mintInCallback += 1;
      if (!mints.length) stats.noMintAnywhere += 1;
      for (const url of allUrls) {
        try {
          const host = new URL(url).host.replace(/^www\./, "");
          stats.hosts[host] = (stats.hosts[host] ?? 0) + 1;
        } catch {
          stats.hosts["(invalid)"] = (stats.hosts["(invalid)"] ?? 0) + 1;
        }
      }

      const record = {
        channel,
        messageId: raw.id,
        date: raw.date ? new Date(raw.date * 1000).toISOString() : null,
        text,
        textLength: text.length,
        entities,
        buttons,
        surfaces: {
          entityUrls: [...new Set(entityUrls)],
          hiddenUrls,
          buttonUrls,
          callbackData
        },
        mints: {
          fromUrls: urlMints,
          fromCallback: [...new Set([...callbackMints, ...callbackUrlMints])],
          fromText: textMints
        },
        hasMedia: Boolean(raw.media),
        views: raw.views ?? null,
        forwards: raw.forwards ?? null
      };
      lines.push(JSON.stringify(record));
      stats.newRecords += 1;
    }

    if (lines.length) {
      await appendFile(path, lines.join("\n") + "\n");
    }
    grandTotal += stats.newRecords;
    summary.channels[channel] = stats;
    console.log(
      `[${channel}] fetched=${stats.fetched} new=${stats.newRecords} ` +
        `mint(url)=${stats.mintInUrl} mint(cb)=${stats.mintInCallback} ` +
        `mint(text)=${stats.mintInText} none=${stats.noMintAnywhere}`
    );
  }

  summary.finishedAt = new Date().toISOString();
  summary.totalNewRecords = grandTotal;
  const summaryPath = join(captureDir, `summary-${Date.now()}.json`);
  await writeFile(summaryPath, JSON.stringify(summary, null, 2));
  console.log(`\nwrote ${grandTotal} new records → ${captureDir}`);
  console.log(`summary → ${summaryPath}`);

  await client.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
