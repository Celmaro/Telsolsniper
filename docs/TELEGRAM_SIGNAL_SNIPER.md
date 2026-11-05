# Telegram Signal Sniper — Integration Plan

This fork adds a separate application layer around the upstream `sol-trade-sdk`.
The DEX SDK remains reusable and its protocol implementations are not rewritten
for Telegram-specific behavior.

## Current first slice

- `telegram-listener/`: TypeScript + Teleproto MTProto listener for the three
  configured public Telegram channels.
- `telegram-listener/src/parser.ts`: extracts a Solana mint address and an
  optional ticker. It performs message-format parsing only; it does not run
  token-risk, profitability, holder, liquidity or AI screening.
- `apps/telegram-signal-sniper/`: Rust Unix-domain-socket receiver and durable
  SQLite signal journal.
- Duplicate Telegram messages are keyed by (source, message_id).
- Duplicate mints across channels are recorded but do not create a second
  entry intent. This defaults to one entry per mint per database.
- The service is observe-only. It does not construct, sign or broadcast trades.

## Why execution is still disabled

A CA and ticker identify the token but do not identify enough transaction
accounts to build every DEX instruction. Before a buy can be constructed, the
execution adapter must resolve the token's actual venue and current pool or
bonding-curve accounts. This is transaction preparation, not token screening.

The execution integration must then:
1. Warm the SDK, RPC client, blockhash cache and fee strategy before signals arrive.
2. Resolve venue/account state through a background refresh path, not by doing
   unbounded RPC discovery inside the signal callback.
3. Build a protocol-specific `SimpleBuyParams` using fresh state and the SDK's
   minimum-output / maximum-input protection.
4. Submit without waiting for final confirmation on the critical path.
5. Reconcile ambiguous submissions by signature before any retry.
6. Track the confirmed fill independently from the submitted transaction.
7. Use actual acquired token quantity for all exits, never an estimated amount.

Until those steps exist and have integration tests, setting
`LIVE_TRADING_ENABLED=true` intentionally makes the Rust service exit with an
error.

## Run the signal intake

Start the Rust service first:

```bash
cargo run -p telegram-signal-sniper
```

In another terminal, configure and start the listener:

```bash
cd telegram-listener
cp .env.example .env
npm install
npm run dev
```

On first run, Teleproto requests a Telegram login code and optional 2FA password.
The session is stored in `telegram-listener/data/telegram.session`; protect it
like an account credential and never commit or share it.

## Environment

Rust service:

- `SIGNAL_SOCKET_PATH`: defaults to `/tmp/telsolsniper.sock`.
- `SIGNAL_DATABASE_PATH`: defaults to `./data/signals.sqlite`.
- `ALLOWED_CHANNELS`: comma-separated usernames; defaults to the three agreed
  channels.
- `LIVE_TRADING_ENABLED`: must remain unset/false until a verified execution
  adapter exists.

Telegram listener:

- `TELEGRAM_API_ID` and `TELEGRAM_API_HASH`: from `my.telegram.org`.
- `TELEGRAM_SESSION_FILE`: local private session file.
- `TELEGRAM_CHANNELS`: comma-separated usernames.
- `SIGNAL_SOCKET_PATH`: must match the Rust service.

## Next engineering milestones

1. Compile and test this intake slice on the target Linux host.
2. Capture representative messages from all three channels and harden parsing
   against their real formats, retaining exact message fixtures without
   introducing token screening.
3. Add durable execution states and recovery semantics.
4. Integrate venue-specific SDK order preparation and submission.
5. Add independent position monitoring and exit execution.
6. Benchmark Telegram receive → parse → IPC → transaction submit, reporting
   p50/p95/p99 and separately measuring on-chain landing/confirmation.

A sub-second submit target is an optimization objective, not a guarantee of
sub-second on-chain inclusion.
