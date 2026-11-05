#!/usr/bin/env python3
"""Telsolsniper shadow stats: read data/shadow.sqlite, print a performance report.

Usage: apps/shadow-engine/stats.py [path-to-shadow.sqlite]
"""
import sqlite3
import statistics
import sys
from pathlib import Path

DEFAULT_DB = Path(__file__).resolve().parent.parent.parent / "data" / "shadow.sqlite"


def fmt_sol(x):
    return f"{x:+.4f} SOL" if x is not None else "—"


def pct(values, p):
    if not values:
        return 0.0
    values = sorted(values)
    idx = min(int(len(values) * p), len(values) - 1)
    return values[idx]


def main():
    db_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_DB
    if not db_path.exists():
        print(f"no database at {db_path}")
        sys.exit(1)
    db = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    db.row_factory = sqlite3.Row

    trades = db.execute(
        """SELECT id, mint, ticker, source, entry_time_unix_ms, exit_time_unix_ms,
                  entry_price_sol, entry_market_cap_sol, buy_sol_amount, token_amount,
                  peak_price_sol, exit_reason, pnl_sol, pnl_pct,
                  tp1_done, tp2_done, realized_pnl_sol
           FROM shadow_trades"""
    ).fetchall()
    if not trades:
        print("no trades recorded yet")
        return

    closed = [t for t in trades if t["exit_time_unix_ms"] is not None]
    open_trades = [t for t in trades if t["exit_time_unix_ms"] is None]
    wins = [t for t in closed if (t["pnl_sol"] or 0) > 0]

    print("=" * 62)
    print("TELSOLSNIPER SHADOW REPORT")
    print("=" * 62)
    print(f"signals taken: {len(trades)}  (closed: {len(closed)}, open: {len(open_trades)})")
    if closed:
        win_rate = 100.0 * len(wins) / len(closed)
        print(f"win rate:      {win_rate:.0f}%  ({len(wins)}/{len(closed)})")
        pnls = [t["pnl_sol"] for t in closed]
        total = sum(pnls)
        print(f"total PnL:     {fmt_sol(total)}")
        print(f"  mean:        {fmt_sol(statistics.mean(pnls))}")
        print(f"  median:      {fmt_sol(statistics.median(pnls))}")
        print(f"  best:        {fmt_sol(max(pnls))}")
        print(f"  worst:       {fmt_sol(min(pnls))}")
        losses = [p for p in pnls if p <= 0]
        if wins and losses:
            avg_win = statistics.mean(t["pnl_sol"] for t in wins)
            avg_loss = abs(statistics.mean(losses))
            print(f"  avg win/loss ratio: {avg_win / avg_loss:.2f}")
            expectancy = (len(wins) / len(closed)) * avg_win - (len(losses) / len(closed)) * avg_loss
            print(f"  expectancy per trade: {fmt_sol(expectancy)}")
    print()

    # Exit reason breakdown
    print("exit reasons:")
    for row in db.execute(
        """SELECT exit_reason, COUNT(*) n, AVG(pnl_pct) avg_pct, SUM(pnl_sol) total_sol
           FROM shadow_trades WHERE exit_time_unix_ms IS NOT NULL
           GROUP BY exit_reason ORDER BY n DESC"""
    ):
        print(f"  {row['exit_reason']:<14} n={row['n']:<3} avg={row['avg_pct']:+.1f}%  total={fmt_sol(row['total_sol'])}")
    print()

    # Stage funnel
    n_tp1 = sum(1 for t in trades if t["tp1_done"])
    n_tp2 = sum(1 for t in trades if t["tp2_done"])
    print("stage funnel:")
    print(f"  entries:      {len(trades)}")
    print(f"  reached TP1:  {n_tp1}  ({100.0 * n_tp1 / len(trades):.0f}%)")
    print(f"  reached TP2:  {n_tp2}  ({100.0 * n_tp2 / len(trades):.0f}%)")
    print()

    # Per-channel
    print("per channel:")
    for row in db.execute(
        """SELECT source, COUNT(*) n,
                  SUM(CASE WHEN pnl_sol > 0 THEN 1 ELSE 0 END) wins,
                  AVG(pnl_pct) avg_pct, SUM(pnl_sol) total_sol
           FROM shadow_trades WHERE exit_time_unix_ms IS NOT NULL
           GROUP BY source ORDER BY total_sol DESC"""
    ):
        wr = 100.0 * row["wins"] / row["n"] if row["n"] else 0.0
        print(f"  {row['source']:<22} n={row['n']:<3} win={wr:.0f}%  avg={row['avg_pct']:+.1f}%  total={fmt_sol(row['total_sol'])}")
    print()

    # Time to TP1
    tp1_times = [
        (f["fill_time_unix_ms"] - t["entry_time_unix_ms"]) / 1000.0
        for f, t in (
            (db.execute(
                "SELECT fill_time_unix_ms FROM shadow_fills WHERE trade_id=? AND kind='tp1' LIMIT 1",
                (t["id"],),
            ).fetchone(), t)
            for t in trades if t["tp1_done"]
        )
        if f
    ]
    if tp1_times:
        print(f"time to TP1:   median {statistics.median(tp1_times):.0f}s  p90 {pct(tp1_times, 0.9):.0f}s")

    # Peak behaviour: how far does price run after entry (closed trades)
    peaks = [
        t["peak_price_sol"] / t["entry_price_sol"]
        for t in closed
        if t["peak_price_sol"] and t["entry_price_sol"]
    ]
    if peaks:
        print(f"peak multiple: median {statistics.median(peaks):.2f}x  p90 {pct(peaks, 0.9):.2f}x  max {max(peaks):.2f}x")
    print()

    # Open positions
    if open_trades:
        print("open positions:")
        now_ms = db.execute(
            "SELECT CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)"
        ).fetchone()[0]
        for t in open_trades:
            age_min = (now_ms - t["entry_time_unix_ms"]) / 60000.0
            mult = (t["peak_price_sol"] or t["entry_price_sol"]) / t["entry_price_sol"]
            print(f"  {t['ticker'] or '—':<10} {t['mint'][:8]}…  age {age_min:.0f}min  peak {mult:.2f}x  mcap {t['entry_market_cap_sol']:.0f} SOL")

    # Moon bags still open
    bags = db.execute(
        """SELECT t.ticker, t.mint, t.realized_pnl_sol, t.remaining_tokens, t.token_amount
           FROM shadow_trades t
           WHERE t.exit_time_unix_ms IS NULL AND t.tp2_done = 1"""
    ).fetchall()
    if bags:
        print()
        print("moon bags open:")
        for b in bags:
            print(f"  {b['ticker'] or '—':<10} {b['mint'][:8]}…  realized {fmt_sol(b['realized_pnl_sol'])}  bag {b['remaining_tokens'] / b['token_amount'] * 100:.0f}%")


if __name__ == "__main__":
    main()
