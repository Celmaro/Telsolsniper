import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { parseTelegramSignal, parseTelegramSignals, classifySignal } from "./parser.js";

const MINT = "So11111111111111111111111111111111111111112";

interface ChannelFixture {
  text: string;
  buttonUrls: string[];
}

/** Exact captures from soltrenchtrending (text + inline-button URLs). */
const CHANNEL_FIXTURES: ChannelFixture[] = JSON.parse(
  readFileSync(new URL("../test/fixtures/soltrenchtrending.json", import.meta.url), "utf8")
);


function fixtureAt(index: number): ChannelFixture {
  const fixture = CHANNEL_FIXTURES[index];
  assert.ok(fixture, `missing channel fixture at index ${index}`);
  return fixture;
}

test("parses a labelled CA and ticker", () => {
  const result = parseTelegramSignal(
    "@soltrenchtrending",
    101,
    `CA: ${MINT}\nTicker: $SOL`,
    1_800_000_000
  );

  assert.ok(result);
  assert.equal(result.source, "soltrenchtrending");
  assert.equal(result.mint, MINT);
  assert.equal(result.ticker, "SOL");
  assert.equal(result.telegramDateUnixMs, 1_800_000_000_000);
});

test("parses a mint from a Pump.fun link", () => {
  const result = parseTelegramSignal(
    "solwhaletrending",
    102,
    `New token: https://pump.fun/coin/${MINT} $WAVE`
  );

  assert.ok(result);
  assert.equal(result.mint, MINT);
  assert.equal(result.ticker, "WAVE");
});

test("keeps a valid CA even if the ticker is not present", () => {
  const result = parseTelegramSignal("solearlytrending", 103, MINT);
  assert.ok(result);
  assert.equal(result.ticker, null);
});

test("rejects messages without a valid Solana public key", () => {
  assert.equal(
    parseTelegramSignal("soltrenchtrending", 104, "Ticker: $NOCA"),
    null
  );
  assert.equal(
    parseTelegramSignal("soltrenchtrending", 105, "CA: 0OIl-not-base58"),
    null
  );
});

test("rejects invalid source metadata", () => {
  assert.equal(parseTelegramSignal(" ", 106, MINT), null);
  assert.equal(parseTelegramSignal("soltrenchtrending", 0, MINT), null);
});

// --- Real channel format: mint lives in inline-button URLs, not text ---

test("extracts the mint from button URLs when the text has none", () => {
  const fixture = fixtureAt(0);
  assert.ok(
    !/\b[1-9A-HJ-NP-Za-km-z]{32,44}\b/.test(fixture.text),
    "fixture text should not contain the mint; the URL must be the source"
  );

  const signals = parseTelegramSignals(
    "soltrenchtrending",
    201,
    fixture.text,
    1_800_000_001,
    fixture.buttonUrls
  );

  assert.equal(signals.length, 1);
  const [first] = signals;
  assert.ok(first);
  assert.equal(first.mint, "7tRqzDMMYpVzfzSogLH6yQ2eFCrYPVUdeAZf4Hx6pump");
  assert.equal(first.source, "soltrenchtrending");
  assert.equal(first.telegramDateUnixMs, 1_800_000_001_000);
});

test("never mistakes solscan holder-account links for the mint", () => {
  const fixture = fixtureAt(0);
  const holderWallets = fixture.buttonUrls
    .filter((url) => url.includes("solscan.io/account/"))
    .map((url) => url.split("solscan.io/account/")[1]?.split(/[?#]/)[0] ?? "")
    .filter((wallet) => wallet.length > 0);

  assert.ok(holderWallets.length > 0, "fixture must contain holder links");

  const signals = parseTelegramSignals(
    "soltrenchtrending",
    202,
    fixture.text,
    undefined,
    fixture.buttonUrls
  );

  for (const wallet of holderWallets) {
    assert.ok(
      !signals.some((signal) => signal.mint === wallet),
      `holder wallet ${wallet} must not be parsed as a mint`
    );
  }
});

test("a leaderboard post yields one signal per announced mint", () => {
  const leaderboard = fixtureAt(3);
  const signals = parseTelegramSignals(
    "soltrenchtrending",
    203,
    leaderboard.text,
    undefined,
    leaderboard.buttonUrls
  );

  assert.equal(signals.length, 12);
  assert.equal(new Set(signals.map((signal) => signal.mint)).size, 12);
  for (const signal of signals) {
    assert.equal(signal.messageId, 203);
    assert.equal(signal.source, "soltrenchtrending");
  }
});

test("supports launchpad links beyond pump.fun (bags.fm)", () => {
  const fixture = fixtureAt(5);
  const signals = parseTelegramSignals(
    "soltrenchtrending",
    204,
    fixture.text,
    undefined,
    fixture.buttonUrls
  );

  assert.equal(signals.length, 1);
  const [first] = signals;
  assert.ok(first);
  assert.ok(first.mint.endsWith("BAGS"));
});

test("every captured channel post produces at least one mint", () => {
  const misses: number[] = [];
  CHANNEL_FIXTURES.forEach((fixture, index) => {
    const signals = parseTelegramSignals(
      "soltrenchtrending",
      300 + index,
      fixture.text,
      undefined,
      fixture.buttonUrls
    );
    if (signals.length === 0) misses.push(index);
  });

  assert.deepEqual(misses, []);
});

test("hidden-link bot prefixes beyond tts_ resolve to the mint", () => {
  // Captured on solearlytrending / solwhaletrending: ets_, wts_, ett_, ttt_
  const mint = "9GNMJdW9U67eBFP6AhdzDLfMqATqwHA9zMJccuoCpump";
  for (const prefix of ["tts", "ets", "wts", "ett", "ttt", "ttb", "fakevol", "ivb"]) {
    const signals = parseTelegramSignals(
      "solearlytrending",
      500,
      "🏆 Top Early Trending",
      undefined,
      [`https://telegram.me/soul_scanner_bot?start=${prefix}_${mint}-abc123`]
    );
    assert.equal(signals.length, 1, `prefix ${prefix} should resolve`);
    assert.equal(signals[0]!.mint, mint);
  }
});

test("bare ?start=<mint> deep links resolve", () => {
  const mint = "7ymF66Z6VhgNiwadtVhjTtT6U7iQRpSxeiRrhfufhigh";
  const signals = parseTelegramSignals(
    "solearlytrending",
    501,
    "alert",
    undefined,
    [`https://t.me/soul_scanner_bot?start=${mint}`]
  );
  assert.equal(signals.length, 1);
  assert.equal(signals[0]!.mint, mint);
});

test("leaderboard post announces every mint in order", () => {
  const first = "7ymF66Z6VhgNiwadtVhjTtT6U7iQRpSxeiRrhfufhigh";
  const second = "Hbpjmu51QXzwojmtvnzzFtwpgYdP8SUwxTTDACGxpump";
  const signals = parseTelegramSignals(
    "solearlytrending",
    502,
    "🏆 Top Early Trending\n🥇 $HIGH • 50X\n🥈 $GEORGE • 15X",
    undefined,
    [
      `https://telegram.me/soul_scanner_bot?start=ets_${first}-ref`,
      `https://telegram.me/soul_scanner_bot?start=ets_${second}-ref`
    ]
  );
  assert.deepEqual(
    signals.map((signal) => signal.mint),
    [first, second]
  );
  assert.equal(signals[0]!.ticker, "HIGH");
});

test("classifies entry vs victory lap vs leaderboard vs follow-up", () => {
  assert.equal(
    classifySignal("🔥 \u200eTHE WLFI Files New Whale Buy!\n🐳 Wallet: 195 SOL"),
    "entry"
  );
  assert.equal(
    classifySignal("🌊🐳 Another Whale Aped \u200e$TIRED!\n🐳 Wallet: 194 SOL"),
    "followup"
  );
  assert.equal(
    classifySignal("📈 \u200eTIRED is up 2X 📈\nfrom ⚡️ Entry Signal\n\n$90.7K —> $182K 💵"),
    "victory_lap"
  );
  assert.equal(
    classifySignal("🏆 Top Whale Trending 🐳\n\n🥇 \u200eDessa | $\u200eDESSA • \u200e301X [X]"),
    "leaderboard"
  );
});

test("24hr stats posts are recaps, not entries", () => {
  for (const text of [
    "💰 24hr Trench Trending Stats 💰",
    "💰 24hr Whale Trending Stats 💰",
  ]) {
    assert.equal(classifySignal(text), "recap", text);
  }
});

test("victory laps are never classified as entries", () => {
  // "Entry Signal" appears in the victory-lap template, so ordering matters.
  for (const text of [
    "📈 \u200eCOGE is up 54% 📈\nfrom ⚡️ Entry Signal",
    "📈 \u200eTIRED is up 50% 📈\nfrom ⚡️ Entry Signal\n\n$90.7K —> $137K 💵",
  ]) {
    assert.equal(classifySignal(text), "victory_lap", text);
  }
});

test("multi-token post prefers the mint in the channel's own bot links", () => {
  // Real capture: solearlytrending message 484519 (Chrome-Chan). The post
  // carried one pump.fun link for an unrelated coin and four bot action links
  // for the announced token. Taking the first mint seen bought the wrong coin.
  const announced = "Fzjm4mxAPbTqxYdTrgRisbXVRPZcG9oeFzCzDxiQpump";
  const unrelated = "CyvQzpKzVGnF4wPGsf9Q8cvWLCfsMLL6Hykp6RwMpump";
  const text = "🔥 \u200eChrome-Chan New Trending\n🕒 Age: 2m | Security: 🚨";
  const links = [
    `https://telegram.me/Soul_Sniper_Bot?start=15_etb_${announced}`,
    `https://pump.fun/coin/${unrelated}`,
    `https://soulterminal.ai/token/${announced}`,
    `https://telegram.me/soul_scanner_bot?start=fakevol_${announced}`,
    `https://telegram.me/Soul_Sniper_Bot?start=ett_track_${announced}`,
  ];
  const parsed = parseTelegramSignals("solearlytrending", 484519, text, 1, links);
  assert.equal(parsed[0]?.mint, announced, "must buy the announced token, not the side link");
  assert.ok(parsed.some((entry) => entry.mint === unrelated), "other mints are still reported");
});

test("every parsed signal carries its kind", () => {
  const mint = "Fzjm4mxAPbTqxYdTrgRisbXVRPZcG9oeFzCzDxiQpump";
  const parsed = parseTelegramSignals(
    "solwhaletrending",
    1,
    `🔥 New Whale Buy!\n💸 2 SOL \u200e$ABC\nhttps://pump.fun/coin/${mint}`,
    1
  );
  assert.equal(parsed[0]?.kind, "entry");
});
