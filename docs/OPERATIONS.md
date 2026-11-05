# Operations — journal, reconciliation, exits, feeds

## 1. Order journal (`order_journal` in `data/live.sqlite`)

Every order is written **before** it is submitted, so a crash can never leave a
transaction on chain that the bot has no record of.

| state | meaning | action |
|---|---|---|
| `intended` | row written, nothing sent yet | reconciler retires it (buy) or flags it (sell) |
| `submitted` | signature returned by the node | reconciler polls the chain |
| `confirmed` | `getTransaction` succeeded; real lamport delta stored | safe to book PnL |
| `failed` | chain rejected it, or it was never submitted | error text stored (redacted) |
| `unknown` | no signature, or the cluster has no record after 5 min | **needs a human** |

Reconciliation runs every `LIVE_RECONCILE_SECS` (30s) over orders older than
20s, independently of trading. `unknown` sells raise an ERROR log; `unknown`
buys are retired automatically. Because it is a separate task, a stuck order
never blocks entries.

## 2. Accounting and PnL safeguards

- Cost and proceeds come from **confirmed journal lamport deltas**, not modeled
  prices. A trade's PnL is `confirmed_sell_lamports − confirmed_buy_lamports`.
- `store.has_unresolved(mint)` pauses exits on that position until the journal
  settles — an in-flight sell may already have executed.
- A position is only closed as `empty` when its entry is confirmed **and** the
  entry order is older than 45s, so RPC lag cannot orphan a live position.
- Entries no longer block on confirmation: submit returns in ~35ms, and the
  reconciler writes the real cost and token balance afterwards.

## 3. Independent exit task

`exit_loop` polls open positions every `LIVE_POLL_INTERVAL_SECS` in its own
task; the entry loop only consumes signals (`LIVE_ENTRY_TICK_MS`, default 250ms).
A slow or hung buy cannot delay a stop. Each entry attempt is additionally
bounded by `LIVE_ENTRY_TIMEOUT_MS` (2500ms).

## 4. Feed monitoring (`telsolsniper-feed.service`)

`apps/feed-monitor` writes `data/feed-health.jsonl` every 5s and a status
snapshot to `data/feed-status.json`:

- per-endpoint slot, block height, latency, slot gap, stalled seconds
- `all_endpoints_down` when no endpoint answers
- per-channel signal age from `data/signals.sqlite`
- event-socket presence

Yellowstone gRPC is not wired (no token provisioned); until then the RPC slot
stream is the continuity source. The continuity maths (stall / gap / height lag)
is independent of transport, so a Yellowstone slot stream slots in unchanged.

## 5. Venue resolver probe (`apps/venue-probe`)

```bash
cargo run -p venue-probe -- --mint <mint>          # one mint
cargo run -p venue-probe -- --file mints.txt       # batch
cargo run -p venue-probe -- --socket /tmp/telsolsniper-events.sock   # live
```

Logs `venue`, `resolve_ms`, `price_sol`, `complete` and `note` to
`data/venue-probe.jsonl`. Venues: `pumpfun_curve`, `pumpswap`,
`pumpswap_unreadable`, `graduated_no_pool`, `unknown` (other launchpad).

## 6. Audit

```bash
python3 apps/live-engine/audit.py
```

Order states, submit latency, signal→trade latency, venue resolution latency,
RPC latency and systemd restart counters.

## 7. Redaction

`redact::redact_url` / `redact_error` strip API keys from RPC URLs (path or
`?api-key=`) and scrub the configured private key from any error text before it
reaches logs, SQLite or the journal. Applied to every trading and reconciler
error path.
