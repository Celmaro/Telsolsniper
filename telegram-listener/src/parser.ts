import bs58 from "bs58";

export interface ParsedTelegramSignal {
  source: string;
  messageId: number;
  mint: string;
  ticker: string | null;
  messageText: string;
  telegramDateUnixMs: number | null;
  receivedAtUnixMs: number;
}

const BASE58_ADDRESS = /(?<![1-9A-HJ-NP-Za-km-z])[1-9A-HJ-NP-Za-km-z]{32,44}(?![1-9A-HJ-NP-Za-km-z])/g;

/**
 * URL shapes that carry the *mint* of the token being announced.
 *
 * Deliberately an allowlist: a generic base58 scan over URLs picks up
 * `solscan.io/account/<wallet>` holder links, which are wallets, not mints.
 * Real captures from soltrenchtrending contain ~20 such links per post.
 */
const MINT_URL_PATTERNS: RegExp[] = [
  /soulterminal\.ai\/token\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  /solscan\.io\/token\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  /pump\.fun\/coin\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  /bags\.fm\/([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  // Telegram bot deep links. Captured prefixes on these channels:
  // tts_, wts_, ets_, ett_, ttt_, wtt_, ivb_, ivf_, fakevol_, ttb_, 56_ttb_
  // (every one is `<prefix>_<mint>`), so match any prefix rather than
  // maintaining a list that silently misses new ones.
  /[?&]start=(?:[A-Za-z0-9]+_)+([1-9A-HJ-NP-Za-km-z]{32,44})/i,
  // Bare `?start=<mint>` and `<mint>-<refcode>` variants.
  /[?&]start=([1-9A-HJ-NP-Za-km-z]{32,44})(?:[-_][A-Za-z0-9]*)?/i,
];

function isSolanaPublicKey(candidate: string): boolean {
  try {
    return bs58.decode(candidate).length === 32;
  } catch {
    return false;
  }
}

function extractMintsFromText(messageText: string): string[] {
  const found: string[] = [];
  const add = (value: string | undefined): void => {
    if (value && isSolanaPublicKey(value) && !found.includes(value)) found.push(value);
  };

  const launchpadLink = messageText.match(
    /(?:pump\.fun\/coin|dexscreener\.com\/solana|solscan\.io\/token)\/([1-9A-HJ-NP-Za-km-z]{32,44})/i
  );
  add(launchpadLink?.[1]);

  const labelled = messageText.match(
    /\b(?:CA|contract(?:\s+address)?|mint|token(?:\s+address)?)\b\s*[:：=\-]?\s*([1-9A-HJ-NP-Za-km-z]{32,44})/i
  );
  add(labelled?.[1]);

  for (const candidate of messageText.matchAll(BASE58_ADDRESS)) {
    add(candidate[0]);
  }
  return found;
}

function extractMintsFromUrls(buttonUrls: string[]): string[] {
  const found: string[] = [];
  const add = (value: string | undefined): void => {
    if (value && isSolanaPublicKey(value) && !found.includes(value)) found.push(value);
  };

  for (const url of buttonUrls) {
    for (const pattern of MINT_URL_PATTERNS) {
      for (const match of url.matchAll(new RegExp(pattern.source, "gi"))) {
        add(match[1]);
      }
    }
  }
  return found;
}

function extractTicker(messageText: string): string | null {
  const explicit = messageText.match(
    /\b(?:ticker|symbol)\b\s*[:：=]\s*\$?([A-Za-z][A-Za-z0-9]{0,14})\b/i
  );
  const cashtag = messageText.match(/\$([A-Za-z][A-Za-z0-9]{0,14})\b/);
  const value = explicit?.[1] ?? cashtag?.[1];
  return value ? value.toUpperCase() : null;
}

/**
 * Parse every mint a Telegram post announces.
 *
 * Real channel posts put the mint in inline-button URLs, not in the message
 * text, and a "trending list" post can announce several mints at once.
 * Returns one entry per unique mint, in discovery order (text first, then
 * button links).
 */
export function parseTelegramSignals(
  source: string,
  messageId: number,
  messageText: string,
  telegramDateUnixSeconds?: number,
  buttonUrls: string[] = []
): ParsedTelegramSignal[] {
  const normalizedSource = source.trim().replace(/^@/, "").toLowerCase();
  const normalizedText = (messageText ?? "").trim();

  if (!normalizedSource || !Number.isSafeInteger(messageId) || messageId <= 0) {
    return [];
  }

  const mints = extractMintsFromText(normalizedText);
  for (const mint of extractMintsFromUrls(buttonUrls)) {
    if (!mints.includes(mint)) mints.push(mint);
  }
  if (mints.length === 0) return [];

  const ticker = extractTicker(normalizedText);
  const telegramDateUnixMs =
    telegramDateUnixSeconds && telegramDateUnixSeconds > 0
      ? telegramDateUnixSeconds * 1000
      : null;
  const receivedAtUnixMs = Date.now();

  return mints.map((mint) => ({
    source: normalizedSource,
    messageId,
    mint,
    ticker,
    messageText: normalizedText.slice(0, 8192),
    telegramDateUnixMs,
    receivedAtUnixMs,
  }));
}

/** Single-signal convenience wrapper: first mint announced by the post. */
export function parseTelegramSignal(
  source: string,
  messageId: number,
  messageText: string,
  telegramDateUnixSeconds?: number,
  buttonUrls: string[] = []
): ParsedTelegramSignal | null {
  return (
    parseTelegramSignals(
      source,
      messageId,
      messageText,
      telegramDateUnixSeconds,
      buttonUrls
    )[0] ?? null
  );
}
