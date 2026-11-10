import { createInterface } from "node:readline/promises";
import { mkdir, readFile, writeFile, chmod } from "node:fs/promises";
import { dirname } from "node:path";
import { TelegramClient } from "teleproto";
import { StringSession } from "teleproto/sessions";
import { NewMessage } from "teleproto/events";
import { loadConfig } from "./config.js";
import { parseTelegramSignals } from "./parser.js";
import { SignalSocketPublisher } from "./signal-socket.js";

interface TelegramMessageButton {
  url?: string;
}

/** Hidden links (MessageEntityTextUrl) carry the mint on real posts. */
interface TelegramMessageEntity {
  className?: string;
  url?: string;
}

interface TelegramMessageUpdate {
  message?: {
    id?: number;
    message?: string;
    date?: number;
    /** Teleproto custom Message getter (already flattened into rows). */
    buttons?: TelegramMessageButton[][];
    entities?: TelegramMessageEntity[];
    /** Raw TL reply markup, as delivered by the update pipeline. */
    replyMarkup?: {
      rows?: Array<{ buttons?: TelegramMessageButton[] }>;
    };
  };
}

/** Inline-button URLs carry the mint on the real channel posts. */
/**
 * URLs hidden behind text (MessageEntityTextUrl). The trending channels use
 * these for the actual token link: `…?start=tts_<mint>` — capture showed
 * 82% of posts carry the mint this way, versus 70% in button URLs.
 */
function collectHiddenUrls(entities: TelegramMessageEntity[] | undefined): string[] {
  const urls: string[] = [];
  for (const entity of entities ?? []) {
    if (entity?.url && typeof entity.url === "string") urls.push(entity.url);
  }
  return urls;
}

function collectButtonUrls(
  buttons: TelegramMessageButton[][] | undefined,
  replyMarkup: { rows?: Array<{ buttons?: TelegramMessageButton[] }> } | undefined
): string[] {
  const urls: string[] = [];
  for (const row of buttons ?? []) {
    for (const button of row) {
      if (typeof button?.url === "string" && button.url.length > 0) {
        urls.push(button.url);
      }
    }
  }
  for (const row of replyMarkup?.rows ?? []) {
    for (const button of row.buttons ?? []) {
      if (typeof button?.url === "string" && button.url.length > 0) {
        urls.push(button.url);
      }
    }
  }
  return urls;
}

async function loadSession(path: string): Promise<string> {
  try {
    return (await readFile(path, "utf8")).trim();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return "";
    throw error;
  }
}

async function main(): Promise<void> {
  const config = loadConfig();
  const sessionValue = await loadSession(config.sessionFile);
  const client = new TelegramClient(
    new StringSession(sessionValue),
    config.apiId,
    config.apiHash,
    { connectionRetries: 5 }
  );

  if (!sessionValue) {
    const readline = createInterface({
      input: process.stdin,
      output: process.stdout
    });
    try {
      await client.start({
        phoneNumber: () => readline.question("Telegram phone number: "),
        password: () => readline.question("Telegram 2FA password (leave blank if none): "),
        phoneCode: () => readline.question("Telegram login code: "),
        onError: (error) => console.error("Telegram authentication error:", error)
      });
    } finally {
      readline.close();
    }
  } else {
    await client.start({
      phoneNumber: async () => {
        throw new Error("Saved Telegram session was rejected; remove the session file and sign in again");
      },
      password: async () => {
        throw new Error("Saved Telegram session requires re-authentication");
      },
      phoneCode: async () => {
        throw new Error("Saved Telegram session requires re-authentication");
      },
      onError: (error) => console.error("Telegram connection error:", error)
    });
  }

  await mkdir(dirname(config.sessionFile), { recursive: true });
  await writeFile(config.sessionFile, client.session.save(), { mode: 0o600 });
  await chmod(config.sessionFile, 0o600);

  const publisher = new SignalSocketPublisher(config.socketPath);
  await publisher.connect();

  console.info("Telegram listener connected to Rust signal intake.");
  console.info("Watching channels:", config.channels.join(", "));

  // ---------------------------------------------------------------------
  // Two delivery paths, first arrival wins.
  //
  // `updates.watch()` polls updates.getChannelDifference: it is the reliable
  // path (no server-side push required, works without joining) but its floor
  // is channelPollInterval = 1s, so a post can sit unseen for up to a second.
  //
  // The MTProto push path delivers the same post over the socket in ms, but
  // has a documented 10-20s tail on some channels (teleproto #36). Running
  // both and deduping on (channel, message id) means we take the fast one
  // when push works and still get the message when only polling works.
  //
  // Both paths log lag = receipt - message.date. message.date is whole
  // seconds, so lag_s is quantised to ~1s; it is still enough to show which
  // path arrives first.
  const seenMessages = new Map<string, number>();
  const DEDUPE_TTL_MS = 10 * 60 * 1000;

  function alreadyHandled(channel: string, messageId: number): boolean {
    const key = `${channel}:${messageId}`;
    const now = Date.now();
    if (seenMessages.has(key)) return true;
    seenMessages.set(key, now);
    if (seenMessages.size > 4096) {
      for (const [k, at] of seenMessages) {
        if (now - at > DEDUPE_TTL_MS) seenMessages.delete(k);
      }
    }
    return false;
  }

  const handleMessage = (channel: string, message: any, via: "push" | "poll"): void => {
      if (!message || typeof message.id !== "number") return;
      if (alreadyHandled(channel, message.id)) return;
      const lagS = typeof message.date === "number" ? Math.round((Date.now() / 1000 - message.date) * 10) / 10 : -1;

      const linkUrls = [
        ...collectHiddenUrls(message.entities),
        ...collectButtonUrls(message.buttons, message.replyMarkup)
      ];

      // A "Top N trending" post announces a leaderboard, not one token.
      // Taking every mint would open N positions off a single post, so cap
      // it (SIGNAL_MAX_MINTS_PER_MESSAGE, default 1 = the rank-#1 token).
      const parseable = parseTelegramSignals(
        channel,
        message.id,
        message.message ?? "",
        message.date,
        linkUrls
      );

      const maxMints = config.maxMintsPerMessage;
      const kind = parseable[0]?.kind ?? "other";
      // Only an entry post announces a token the channel is calling now.
      // Follow-ups, victory laps and leaderboard recaps reference positions
      // already called: buying those means buying after the move.
      const tradeable = config.tradeKinds.has(kind);
      const signals = tradeable ? parseable.slice(0, maxMints) : [];
      if (parseable.length > 0 && !tradeable) {
        console.info(
          `not an entry post source=${channel} message=${message.id} kind=${kind} mints=${parseable.length} via=${via} lag_s=${lagS}`
        );
      }
      if (parseable.length > maxMints) {
        console.info(
          `capped source=${channel} message=${message.id} announced=${parseable.length} taking=${maxMints}`
        );
      }

      for (const signal of signals) {
        void publisher.publish(signal).then(
          () => {
            console.info(
              `signal forwarded source=${signal.source} message=${signal.messageId} mint=${signal.mint} ticker=${signal.ticker ?? "n/a"} via=${via} lag_s=${lagS}`
            );
          },
          (error: unknown) => {
            console.error("Could not forward Telegram signal:", error);
          }
        );
      }
  };

  const stopWatchers = config.channels.map((channel) =>
    client.updates.watch(channel, (update) => {
      const message = (update as TelegramMessageUpdate).message;
      handleMessage(channel, message, "poll");
    })
  );

  // Push path. NewMessage fires straight off the socket; if the account is not
  // a member of a channel this simply never fires and polling stays in charge.
  client.addEventHandler(async (event: any) => {
    const message = event?.message;
    if (!message) return;
    let username: string | undefined;
    try {
      const chat = await message.getChat();
      username = chat?.username;
    } catch {
      return;
    }
    if (!username || !config.channels.includes(username)) return;
    handleMessage(username, message, "push");
  }, new NewMessage({}));

  const shutdown = async (): Promise<void> => {
    for (const stop of stopWatchers) stop();
    await client.disconnect();
    process.exit(0);
  };
  process.once("SIGINT", () => void shutdown());
  process.once("SIGTERM", () => void shutdown());
}

main().catch((error: unknown) => {
  console.error("Telegram listener failed:", error);
  process.exitCode = 1;
});
