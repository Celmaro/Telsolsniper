#!/usr/bin/env python3
"""Speed / latency / stability audit for Telsolsniper.

Reads only artifacts the system already writes:
  data/live.sqlite        trades, fills, order_journal
  data/venue-probe.jsonl  venue resolution latency
  data/feed-health.jsonl  RPC latency and continuity
and systemd restart counters.

Usage: python3 apps/live-engine/audit.py
"""
import json
import os
import statistics
import subprocess
import sqlite3
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path("/home/agentuser/Telsolsniper")
LIVE_DB = ROOT / "data" / "live.sqlite"
VENUE_LOG = ROOT / "data" / "venue-probe.jsonl"
FEED_LOG = ROOT / "data" / "feed-health.jsonl"


def pct(values, p):
    if not values:
        return None
    values = sorted(values)
    index = min(int(len(values) * p), len(values) - 1)
    return values[index]


def q(values, unit=""):
    if not values:
        return "no data"
    return (
        f"p50 {pct(values,0.50):.0f}{unit}  p95 {pct(values,0.95):.0f}{unit}  "
        f"max {max(values):.0f}{unit}  (n={len(values)})"
    )


def systemd_restarts(unit):
    try:
        out = subprocess.run(
            ["systemctl", "--user", "show", unit, "-p", "NRestarts", "-p", "ActiveEnterTimestamp"],
            capture_output=True, text=True, timeout=10,
        ).stdout
        values = dict(line.split("=", 1) for line in out.strip().splitlines() if "=" in line)
        return values.get("NRestarts", "?"), values.get("ActiveEnterTimestamp", "?")
    except Exception:
        return "?", "?"


def main():
    print("=" * 78)
    print("TELSOLSNIPER AUDIT — latency, throughput, stability")
    print("=" * 78)

    # ---------- pipeline latency from the journal ----------
    if LIVE_DB.exists():
        db = sqlite3.connect(f"file:{LIVE_DB}?mode=ro", uri=True)
        db.row_factory = sqlite3.Row

        rows = db.execute(
            """SELECT j.id, j.mint, j.side, j.kind, j.state, j.created_ms, j.updated_ms,
                      j.submitted_ms, j.reconciled_ms, j.lamports, j.error, t.entry_time_unix_ms
               FROM order_journal j LEFT JOIN live_trades t ON t.mint = j.mint
               ORDER BY j.id"""
        ).fetchall()
        states = Counter(r["state"] for r in rows)
        print(f"\nORDER JOURNAL  ({len(rows)} orders)")
        for state, count in states.most_common():
            print(f"  {state:<10} {count}")
        unknown = [r for r in rows if r["state"] == "unknown"]
        if unknown:
            print(f"  ⚠ {len(unknown)} UNKNOWN orders need manual review:")
            for r in unknown[:5]:
                print(f"     {r['mint'][:10]}… {r['side']}/{r['kind']} — {str(r['error'])[:60]}")

        # submit latency: intent -> updated (submitted/confirmed)
        submit = [
            r["submitted_ms"] - r["created_ms"]
            for r in rows
            if r["created_ms"] and r["submitted_ms"]
        ]
        ok_submit = [s for s in submit if 0 <= s < 60_000]
        print(f"\nSUBMIT LATENCY (intent → single-node response)")
        print(f"  {q(ok_submit,' ms')}")

        # confirm latency proxy: created -> reconciled not stored separately;
        # use updated-created for confirmed rows as upper bound
        confirmed = [
            (r["reconciled_ms"] or r["updated_ms"]) - r["created_ms"]
            for r in rows
            if r["state"] == "confirmed"
        ]
        print(f"CONFIRM BOUND (intent → reconciled, upper bound)")
        print(f"  {q(confirmed,' ms')}")

        # signal → entry: intake received_at vs trade entry_time
        try:
            sig = sqlite3.connect(f"file:{ROOT/'data'/'signals.sqlite'}?mode=ro", uri=True)
            sig.row_factory = sqlite3.Row
            lat = []
            for trade in db.execute("SELECT mint, entry_time_unix_ms FROM live_trades"):
                match = sig.execute(
                    "SELECT received_at_unix_ms FROM telegram_messages WHERE mint = ?1 ORDER BY received_at_unix_ms LIMIT 1",
                    (trade["mint"],),
                ).fetchone()
                if match and trade["entry_time_unix_ms"]:
                    delta = trade["entry_time_unix_ms"] - match["received_at_unix_ms"]
                    if 0 <= delta < 120_000:
                        lat.append(delta)
            print(f"\nEND-TO-END: telethon receive → trade recorded")
            print(f"  {q(lat,' ms')}")
        except Exception as error:
            print(f"\nEND-TO-END: unavailable ({error})")

        # fills / accounting
        fills = db.execute("SELECT kind, proceeds_lamports, multiple FROM live_fills").fetchall()
        if fills:
            print(f"\nFILLS ({len(fills)})")
            for kind, count in Counter(f["kind"] for f in fills).most_common():
                print(f"  {kind:<10} {count}")
        trades = db.execute(
            "SELECT COUNT(*) n, COALESCE(SUM(pnl_sol),0) pnl FROM live_trades"
        ).fetchone()
        print(f"\nTRADES: {trades['n']}  net PnL {trades['pnl']:+.6f} SOL")

    # ---------- venue resolution latency ----------
    if VENUE_LOG.exists():
        records = [json.loads(line) for line in open(VENUE_LOG)]
        by_venue = defaultdict(list)
        for record in records:
            by_venue[record["venue"]].append(record["resolve_ms"])
        print(f"\nVENUE RESOLUTION LATENCY ({len(records)} probes)")
        for venue, values in sorted(by_venue.items(), key=lambda kv: -len(kv[1])):
            print(f"  {venue:<20} n={len(values):<3} {q(values,' ms')}")
        slow = [r for r in records if r["resolve_ms"] > 2000]
        if slow:
            print(f"  ⚠ {len(slow)}/{len(records)} resolutions exceeded 2s — this is the hot-path bottleneck")

    # ---------- RPC health ----------
    if FEED_LOG.exists():
        ticks = [json.loads(line) for line in open(FEED_LOG)]
        latency = defaultdict(list)
        unhealthy = Counter()
        for tick in ticks[-200:]:
            for endpoint, status in (tick.get("endpoints") or {}).items():
                if "latency_ms" in status:
                    latency[endpoint].append(status["latency_ms"])
                if status.get("consecutive_failures", 0) > 0:
                    unhealthy[endpoint] += 1
        print(f"\nRPC LATENCY (last {min(len(ticks),200)} ticks)")
        for endpoint, values in latency.items():
            print(f"  {endpoint:<35} {q(values,' ms')}")
        if unhealthy:
            print(f"  failures: {dict(unhealthy)}")

    # ---------- stability ----------
    print("\nSTABILITY (systemd)")
    for unit in (
        "telsolsniper-listener",
        "telsolsniper-intake",
        "telsolsniper-shadow",
        "telsolsniper-live",
    ):
        restarts, active_since = systemd_restarts(unit)
        print(f"  {unit:<24} restarts={restarts:<3} since={active_since}")


if __name__ == "__main__":
    main()
