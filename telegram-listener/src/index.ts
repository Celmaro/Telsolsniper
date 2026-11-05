import { createInterface } from "node:readline/promises";
import { mkdir, readFile, writeFile, chmod } from "node:fs/promises";
import { dirname } from "node:path";
import { TelegramClient } from "teleproto";
import { StringSession } from "teleproto/sessions";
import { loadConfig } from "./config.js";
import { parseTelegramSignals } from "./parser.js";
import { SignalSocketPublisher } from "./signal-socket.js";

interface TelegramMessageButton {
  url?: string;
}

interface TelegramMessageUpdate {
  message?: {
    id?: number;
    message?: string;
    date?: number;
    /** Teleproto custom Message getter (already flattened into rows). */
    buttons?: TelegramMessageButton[][];
    /** Raw TL reply markup, as delivered by the update pipeline. */
    replyMarkup?: {
      rows?: Array<{ buttons?: TelegramMessageButton[] }>;
    };
  };
}

/** Inline-button URLs carry the mint on the real channel posts. */
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

  const stopWatchers = config.channels.map((channel) =>
    client.updates.watch(channel, (update) => {
      const message = (update as TelegramMessageUpdate).message;
      if (!message || typeof message.id !== "number") {
        return;
      }

      const signals = parseTelegramSignals(
        channel,
        message.id,
        message.message ?? "",
        message.date,
        collectButtonUrls(message.buttons, message.replyMarkup)
      );

      for (const signal of signals) {
        void publisher.publish(signal).then(
          () => {
            console.info(
              `signal forwarded source=${signal.source} message=${signal.messageId} mint=${signal.mint} ticker=${signal.ticker ?? "n/a"}`
            );
          },
          (error: unknown) => {
            console.error("Could not forward Telegram signal:", error);
          }
        );
      }
    })
  );

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
