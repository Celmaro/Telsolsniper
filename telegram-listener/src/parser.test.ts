import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { parseTelegramSignal, parseTelegramSignals } from "./parser.js";

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
