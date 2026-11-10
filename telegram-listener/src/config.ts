export interface ListenerConfig {
  apiId: number;
  apiHash: string;
  sessionFile: string;
  channels: string[];
  socketPath: string;
  /** Max mints to act on from a single post (list posts announce several). */
  maxMintsPerMessage: number;
  /** Post kinds allowed to trigger a buy (entry, followup, ...). */
  tradeKinds: Set<string>;
}

function required(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

export function loadConfig(): ListenerConfig {
  const apiId = Number(required("TELEGRAM_API_ID"));
  if (!Number.isSafeInteger(apiId) || apiId <= 0) {
    throw new Error("TELEGRAM_API_ID must be a positive integer");
  }

  const channels = (process.env.TELEGRAM_CHANNELS ??
    "soltrenchtrending,solwhaletrending,solearlytrending")
    .split(",")
    .map((channel) => channel.trim().replace(/^@/, "").toLowerCase())
    .filter(Boolean);

  if (channels.length === 0) {
    throw new Error("TELEGRAM_CHANNELS must contain at least one channel");
  }

  return {
    apiId,
    apiHash: required("TELEGRAM_API_HASH"),
    sessionFile: process.env.TELEGRAM_SESSION_FILE ?? "./data/telegram.session",
    channels: [...new Set(channels)],
    socketPath: process.env.SIGNAL_SOCKET_PATH ?? "/tmp/telsolsniper.sock",
    maxMintsPerMessage: Number(process.env.SIGNAL_MAX_MINTS_PER_MESSAGE ?? "1"),
    // Which post types may trigger a buy. Default: entry posts only.
    // Comma-separated so a follow-up can be opted into without a code change.
    tradeKinds: new Set(
      (process.env.SIGNAL_TRADE_KINDS ?? "entry")
        .split(",")
        .map((kind) => kind.trim().toLowerCase())
        .filter(Boolean)
    ),
  };
}
