#!/usr/bin/env python3
"""Telsolsniper live stats — separates strategy edge from fee drag.

Raw PnL at 0.001 SOL is dominated by fixed transaction fees, so this
report also shows the gross multiple captured (size-independent) and
what the same trades would net at larger sizes.

Usage: apps/live-engine/stats.py [--size 0.01]
"""
import statistics
import sqlite3
import sys
from pathlib import Path

DB = Path(__file__).resolve().parent.parent.parent / "data" / "live.sqlite"
TX_FEE_LAMPORTS = 80_000  # measured on mainnet with our CU settings
LAMPORTS = 1_000_000_000


def main():
    target_size = 0.01
    if "--size" in sys.argv:
        target_size = float(sys.argv[sys.argv.index("--size") + 1])

    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    trades = db.execute("SELECT * FROM live_trades ORDER BY id").fetchall()
    if not trades:
        print("no live trades yet")
        return

    print("=" * 72)
    print("TELSOLSNIPER LIVE REPORT   (real money, real signatures)")
    print("=" * 72)

    closed = [t for t in trades if t["exit_time_unix_ms"] is not None]
    open_t = [t for t in trades if t["exit_time_unix_ms"] is None]
    print(f"trades: {len(trades)}  closed: {len(closed)}  open: {len(open_t)}\n")

    if closed:
        pnls = [t["pnl_sol"] or 0.0 for t in closed]
        wins = [p for p in pnls if p > 0]
        print(f"REAL PnL (as traded): {sum(pnls):+.6f} SOL")
        print(f"  mean {statistics.mean(pnls):+.6f}  median {statistics.median(pnls):+.6f}")
        print(f"  best {max(pnls):+.6f}  worst {min(pnls):+.6f}  win rate {100*len(wins)/len(closed):.0f}%")
        print()

    # NOTE: proceeds include refunded ATA rent, so raw proceeds cannot be
    # used to infer price multiples. Strategy quality is reported only via
    # the ladder funnel below, which uses venue prices at decision time.

    print("FEE LOAD BY POSITION SIZE (3 transactions per round trip)")
    for size in (0.001, 0.005, 0.01, 0.05):
        fees = 3 * TX_FEE_LAMPORTS / LAMPORTS
        print(f"  {size:>6.3f} SOL \u2192 fees {fees:.6f} SOL = {100*fees/size:>5.1f}% of position")

    # Ladder hit rates
    n = len(trades)
    tp1 = sum(1 for t in trades if t["tp1_done"])
    tp2 = sum(1 for t in trades if t["tp2_done"])
    print()
    print("LADDER FUNNEL")
    print(f"  entries     {n}")
    print(f"  reached TP1 {tp1}  ({100*tp1/n:.0f}%)")
    print(f"  reached TP2 {tp2}  ({100*tp2/n:.0f}%)")

    stops = sum(1 for t in closed if t["exit_reason"] == "stop")
    bags = sum(1 for t in closed if t["exit_reason"] == "moon_bag")
    empties = sum(1 for t in closed if t["exit_reason"] == "empty")
    print(f"  exits: stop {stops}  moon_bag {bags}  empty/orphan {empties}")

    if open_t:
        print("\nOPEN POSITIONS")
        for t in open_t:
            print(f"  #{t['id']} {t['mint'][:8]}…  tp1={bool(t['tp1_done'])} tp2={bool(t['tp2_done'])}")


if __name__ == "__main__":
    main()
