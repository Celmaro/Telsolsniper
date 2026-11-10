import bs58 from "bs58";

export interface ParsedTelegramSignal {
  source: string;
  messageId: number;
  mint: string;
  ticker: string | null;
  messageText: string;
  telegramDateUnixMs: number | null;
  receivedAtUnixMs: number;
  /** What kind of post this is, so the engines only trade real entries. */
  kind: SignalKind;
}

/**
 * Post types observed on the three channels.
 *
 * Only `entry` announces a token the channel is calling *now*. The rest are
 * commentary on positions it already called: follow-ups ("Another Whale
 * Aped $COGE"), victory laps ("TIRED is up 2X" — buying that means buying the
 * top), and leaderboard recaps. Buying anything but `entry` is a bug.
 */
export type SignalKind =
  | "entry"
  | "followup"
  | "victory_lap"
  | "leaderboard"
  | "recap"
  | "other";

const RECAP_PATTERN = /24hr?\s+.*Trending\s+Stats|\d+hr\s+.*Stats|Stats\s*💰|📊\s*Stats/i;
const LEADERBOARD_PATTERN = /Top\s+(?:Whale|Trench|Early)\s+Trending|🏆\s*Top\s+Trending/i;
const VICTORY_PATTERN =
  /is\s+up\s+\d|from\s+⚡|from\s+.*Entry\s+Signal|📈.*is\s+up|💸💸/i;
const FOLLOWUP_PATTERN =
  /Another\s+Whale\s+Aped|Whale\s+Accumulating|Accumulating|Still\s+Holding|Whale\s+Added|Whale\s+Bought\s+More|Adding\s+More/i;
const ENTRY_PATTERN =
  /New\s+Whale\s+Buy|New\s+Trending|New\s+Early|New\s+Token|New\s+Buy|New\s+Signal|Entry\s+Signal|Just\s+Launched|Fresh\s+Launch|New\s+Gem|New\s+Call/i;

/** Classify a channel post. Order matters: leaderboard and victory laps also
 * contain words like "Trending"/"Entry Signal". */
export function classifySignal(messageText: string): SignalKind {
  const text = messageText ?? "";
  if (RECAP_PATTERN.test(text)) return "recap";
  if (LEADERBOARD_PATTERN.test(text)) return "leaderboard";
  if (VICTORY_PATTERN.test(text)) return "victory_lap";
  if (FOLLOWUP_PATTERN.test(text)) return "followup";
  if (ENTRY_PATTERN.test(text)) return "entry";
  return "other";
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
/**
 * Rank announced mints by how strongly the post points at them.
 *
 * A post can carry links to more than one token: the channel's own bot links
 * (buy/track/scan) all reference the announced token, while an incidental
 * `pump.fun/coin/...` link can reference something else entirely (a related
 * ad, an embed). Taking the first mint seen picked the wrong token in the
 * Chrome-Chan capture: it resolved `CyvQzpKz…` from a single pump.fun link
 * while `Fzjm4mx…` appeared in every bot action link.
 *
 * So: the channel's own `start=` deep links vote strongest, launchpad/explorer
 * links vote weakly, and bare base58 in text votes weakest.
 */
const BOT_LINK_PATTERN = /[?&]start=(?:[A-Za-z0-9]+_)+([1-9A-HJ-NP-Za-km-z]{32,44})|[?&]start=([1-9A-HJ-NP-Za-km-z]{32,44})(?:[-_][A-Za-z0-9]*)?/i;

function mintWeightsFromUrls(urls: string[]): Map<string, number> {
  const scores = new Map<string, number>();
  const bump = (mint: string, weight: number): void => {
    if (!isSolanaPublicKey(mint)) return;
    scores.set(mint, (scores.get(mint) ?? 0) + weight);
  };

  for (const url of urls) {
    let matched = false;
    for (const match of url.matchAll(new RegExp(BOT_LINK_PATTERN.source, "gi"))) {
      const mint = match[1] ?? match[2];
      if (mint) {
        bump(mint, 3); // channel's own action link — strongest signal
        matched = true;
      }
    }
    if (matched) continue;
    for (const pattern of MINT_URL_PATTERNS) {
      for (const match of url.matchAll(new RegExp(pattern.source, "gi"))) {
        const mint = match[1];
        if (mint) bump(mint, 1);
      }
    }
  }
  return scores;
}

/** Merge mint sources into one ranking: highest total weight first, ties keep
 * first-seen order. */
function rankMints(
  scores: Map<string, number>,
  orderedMints: string[]
): string[] {
  const rank = (mint: string): number => scores.get(mint) ?? 0;
  return [...new Set(orderedMints)].sort((left, right) => {
    const diff = rank(right) - rank(left);
    if (diff !== 0) return diff;
    return orderedMints.indexOf(left) - orderedMints.indexOf(right);
  });
}

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

  const textMints = extractMintsFromText(normalizedText);
  const urlMints = extractMintsFromUrls(buttonUrls);
  const ordered = [...textMints, ...urlMints];
  if (ordered.length === 0) return [];

  // Weight by strength of evidence, then de-duplicate.
  const scores = mintWeightsFromUrls(buttonUrls);
  for (const mint of textMints) {
    scores.set(mint, Math.max(scores.get(mint) ?? 0, 1));
  }
  const mints = rankMints(scores, ordered);

  const ticker = extractTicker(normalizedText);
  const telegramDateUnixMs =
    telegramDateUnixSeconds && telegramDateUnixSeconds > 0
      ? telegramDateUnixSeconds * 1000
      : null;
  const receivedAtUnixMs = Date.now();
  const kind = classifySignal(normalizedText);

  return mints.map((mint) => ({
    source: normalizedSource,
    messageId,
    mint,
    ticker,
    messageText: normalizedText.slice(0, 8192),
    telegramDateUnixMs,
    receivedAtUnixMs,
    kind,
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
