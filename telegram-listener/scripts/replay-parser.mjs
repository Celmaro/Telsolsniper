#!/usr/bin/env node
/**
 * Replay captured messages through the real parser and measure coverage.
 * This is the feedback loop: capture -> analyze patterns -> parser change ->
 * replay to prove the change actually helped.
 *
 * Run: npm run replay
 */
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { parseTelegramSignals } from "../src/parser.ts";

const CAPTURE_DIR = join(process.cwd(), "data", "captures");

async function main() {
  const files = (await readdir(CAPTURE_DIR)).filter((name) => name.endsWith(".jsonl"));
  let totalMessages = 0;
  let messagesWithMint = 0;
  let parsedMessages = 0;
  let mintCount = 0;
  let multiMintMessages = 0;
  let missedNoMint = 0;
  const perChannel = {};

  for (const file of files) {
    const channel = file.replace(".jsonl", "");
    const content = await readFile(join(CAPTURE_DIR, file), "utf8");
    const stats = { messages: 0, withMint: 0, parsed: 0, mints: 0, missed: 0 };

    for (const line of content.split("\n")) {
      if (!line.trim()) continue;
      const record = JSON.parse(line);
      stats.messages += 1;
      totalMessages += 1;

      // Ground truth from capture: every mint seen on any surface.
      const truth = new Set([
        ...(record.mints?.fromUrls ?? []),
        ...(record.mints?.fromCallback ?? []),
        ...(record.mints?.fromText ?? [])
      ]);
      if (truth.size > 0) {
        stats.withMint += 1;
        messagesWithMint += 1;
      }
      if (truth.size > 1) multiMintMessages += 1;

      const urls = [
        ...(record.surfaces?.entityUrls ?? []),
        ...(record.surfaces?.hiddenUrls ?? []),
        ...(record.surfaces?.buttonUrls ?? [])
      ];
      const signals = parseTelegramSignals(
        channel,
        record.messageId,
        record.text ?? "",
        record.date ? Math.floor(new Date(record.date).getTime() / 1000) : undefined,
        urls
      );

      if (signals.length > 0) {
        stats.parsed += 1;
        parsedMessages += 1;
        stats.mints += signals.length;
        mintCount += signals.length;
      }

      // Missed = capture found a mint but the parser found none.
      if (truth.size > 0 && signals.length === 0) {
        stats.missed += 1;
        missedNoMint += 1;
        if (stats.missed <= 2) {
          console.log(`  MISS ${channel}/${record.messageId} truth=${[...truth][0].slice(0, 10)}… urls=${urls.length}`);
        }
      }
    }
    perChannel[channel] = stats;
    const recall = stats.withMint ? (100 * (stats.withMint - stats.missed)) / stats.withMint : 0;
    console.log(
      `${channel.padEnd(20)} messages=${String(stats.messages).padStart(3)} ` +
        `withMint=${String(stats.withMint).padStart(3)} parsed=${String(stats.parsed).padStart(3)} ` +
        `mints=${String(stats.mints).padStart(4)} missed=${stats.missed} recall=${recall.toFixed(1)}%`
    );
  }

  const recall = messagesWithMint ? (100 * (messagesWithMint - missedNoMint)) / messagesWithMint : 0;
  console.log("");
  console.log(`TOTAL messages=${totalMessages} withMint=${messagesWithMint} parsed=${parsedMessages}`);
  console.log(`signals emitted=${mintCount} (multi-mint posts=${multiMintMessages})`);
  console.log(`RECALL vs capture ground truth: ${recall.toFixed(1)}%  (missed ${missedNoMint})`);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
