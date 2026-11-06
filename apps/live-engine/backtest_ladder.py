#!/usr/bin/env python3
"""Ladder backtest on recorded price anchors.

Every settled trade gives us up to three observed price points:
  entry (1.00x), the highest fill multiple reached, and the final exit multiple.
That is enough to compare TP levels and stop widths against real trades — but it
does NOT reconstruct intrabar paths, so trailing behaviour is approximated by
the final observed multiple. Use it to reject bad ladders, not to fine-tune.

Fee model is measured, not assumed:
  per tx           0.00011 SOL  (base 5k + priority 75k + tip 20k lamports)
  ATA rent refund  0.001434 SOL returned when the token ATA closes
  position         0.001 SOL (LIVE_BUY_SOL)
"""
import sqlite3
import statistics

DB = "/home/agentuser/Telsolsniper/data/live.sqlite"
POSITION = 0.001
FEE_TX = 0.00011
RENT_REFUND = 0.001434


def load():
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    trades = []
    for t in db.execute(
        "SELECT id, entry_price_sol, exit_reason FROM live_trades WHERE exit_time_unix_ms IS NOT NULL"
    ):
        fills = list(
            db.execute(
                "SELECT kind, multiple, tokens_sold FROM live_fills WHERE trade_id=?", (t["id"],)
            )
        )
        if not fills:
            continue
        multiples = [f["multiple"] for f in fills if f["multiple"]]
        max_mult = max(multiples) if multiples else 1.0
        final = next(
            (f["multiple"] for f in fills if f["kind"] in ("stop", "moon_bag", "stale")),
            min(multiples) if multiples else 1.0,
        )
        trades.append({"id": t["id"], "max": max_mult, "final": final, "reason": t["exit_reason"]})
    return trades


def simulate(trade, tp1_x, tp1_frac, tp2_x, tp2_frac, stop_pct):
    """Return net SOL for one trade under a ladder."""
    remaining = 1.0
    proceeds = 0.0
    sells = 0
    max_mult = trade["max"]
    final_mult = trade["final"]

    if max_mult >= tp1_x:
        sold = remaining * tp1_frac
        proceeds += sold * POSITION * tp1_x
        remaining -= sold
        sells += 1
    if tp2_x > 0 and max_mult >= tp2_x and remaining > 0.01:
        sold = remaining * tp2_frac
        proceeds += sold * POSITION * tp2_x
        remaining -= sold
        sells += 1

    if remaining > 0.01:
        # The remainder exits at the observed final multiple, floored at the
        # stop level (a stop cannot fill better than its trigger).
        exit_mult = max(final_mult, 1.0 - stop_pct / 100.0)
        if trade["reason"] == "empty":
            exit_mult = 0.0
        proceeds += remaining * POSITION * exit_mult
        sells += 1

    # Rent cancels: it leaves the wallet on the buy (creating the token ATA) and
    # comes back on the closing sell. Only the buy amount and tx fees are net.
    cost = POSITION + FEE_TX * (1 + sells)
    return proceeds - cost


def evaluate(trades, name, **ladder):
    pnls = [simulate(t, **ladder) for t in trades]
    wins = sum(1 for p in pnls if p > 0)
    return {
        "name": name,
        "total": sum(pnls),
        "avg": statistics.mean(pnls),
        "median": statistics.median(pnls),
        "wins": wins,
        "n": len(pnls),
        "best": max(pnls),
        "worst": min(pnls),
    }


LADDERS = [
    ("live now: 1.5x/60% no-TP2, stop35", dict(tp1_x=1.5, tp1_frac=0.60, tp2_x=0, tp2_frac=0, stop_pct=35)),
    ("1.5x/70%, stop20", dict(tp1_x=1.5, tp1_frac=0.70, tp2_x=0, tp2_frac=0, stop_pct=20)),
    ("1.5x/70%, stop15", dict(tp1_x=1.5, tp1_frac=0.70, tp2_x=0, tp2_frac=0, stop_pct=15)),
    ("1.5x/100% (no moonbag)", dict(tp1_x=1.5, tp1_frac=1.0, tp2_x=0, tp2_frac=0, stop_pct=20)),
    ("1.3x/70%, stop15", dict(tp1_x=1.3, tp1_frac=0.70, tp2_x=0, tp2_frac=0, stop_pct=15)),
    ("1.3x/100%", dict(tp1_x=1.3, tp1_frac=1.0, tp2_x=0, tp2_frac=0, stop_pct=15)),
    ("1.2x/60% + 1.6x/40%", dict(tp1_x=1.2, tp1_frac=0.60, tp2_x=1.6, tp2_frac=0.40, stop_pct=15)),
    ("old: 1.1x/40% + 1.5x/40%", dict(tp1_x=1.1, tp1_frac=0.40, tp2_x=1.5, tp2_frac=0.40, stop_pct=35)),
]


def main():
    trades = load()
    print(f"backtest universe: {len(trades)} settled trades with observed fills")
    direct = sum(1 for t in trades if t["max"] <= 1.0)
    print(f"  never traded above entry: {direct} ({direct/len(trades)*100:.0f}%)")
    print()
    header = f"{'ladder':<34}{'total':>10}{'avg':>10}{'median':>10}{'wins':>7}"
    print(header)
    print("-" * len(header))
    results = []
    for name, ladder in LADDERS:
        r = evaluate(trades, name, **ladder)
        results.append(r)
        print(f"{name:<34}{r['total']:>+10.5f}{r['avg']:>+10.6f}{r['median']:>+10.6f}{r['wins']:>4}/{r['n']}")
    best = max(results, key=lambda r: r["total"])
    print(f"\nbest by total: {best['name']} ({best['total']:+.5f} SOL)")
    print("caveat: trailing uses the observed final multiple; a different trail %")
    print("would need intrabar price paths, which are not recorded.")


if __name__ == "__main__":
    main()
