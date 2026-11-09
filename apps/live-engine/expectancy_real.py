#!/usr/bin/env python3
"""Real-trade expectancy model.

Unlike backtest_ladder.py (which replayed only observed exit fills), this uses
the REAL peak-multiple distribution from 71 live mainnet trades and the REAL
measured degradation from peak to fill, so it answers: "if we had run ladder X
on the trades we actually got, what would it have returned?"

Measured facts it is built on (live.sqlite, 71 settled trades):
  - peak multiple: p50 1.15x, p75 1.95x, p90 2.64x, p99 16.8x
  - 32% reach 1.5x, 24% reach 2x, 15% never trade above entry
  - fall from peak to the actual exit fill: median 42%, p90 99%

Position 0.01 SOL, fees 0.0000445/tx (base+priority+tip at cu_limit 130k).
"""
import sqlite3
import statistics

DB = "/home/agentuser/Telsolsniper/data/live.sqlite"
SHADOW_DB = "/home/agentuser/Telsolsniper/data/shadow.sqlite"
POSITION = 0.01
FEE_TX = 0.0000445
# Measured: the price at fill is lower than the trigger. A 35% trail with
# 5% slippage plus one poll interval of latency behaves like this.
TRAIL_REALISATION = 0.58  # fraction of peak actually captured by a trailing exit


def real_peaks():
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    peaks = []
    for r in db.execute(
        "select peak_price_sol, entry_price_sol from live_trades "
        "where exit_time_unix_ms is not null and entry_price_sol > 0"
    ):
        peaks.append(r["peak_price_sol"] / r["entry_price_sol"] if r["peak_price_sol"] else 1.0)
    return peaks


def shadow_peaks():
    db = sqlite3.connect(f"file:{SHADOW_DB}?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    peaks = []
    for r in db.execute(
        "select peak_price_sol, entry_price_sol from shadow_trades "
        "where buy_sol_amount=0.01 and exit_time_unix_ms is not null and entry_price_sol > 0"
    ):
        if r["peak_price_sol"]:
            peaks.append(r["peak_price_sol"] / r["entry_price_sol"])
    return peaks


def sim(peak, tp1_x, tp1_frac, trail_capture, stop_pct, time_stop_mult=None):
    """Net SOL for one trade given its observed peak."""
    proceeds = 0.0
    remaining = 1.0
    sells = 0

    if peak >= tp1_x:
        sold = remaining * tp1_frac
        proceeds += sold * POSITION * tp1_x
        remaining -= sold
        sells += 1

    if remaining > 0.001:
        if peak >= tp1_x:
            # Trailing exit on the remainder.
            exit_mult = max(1.0, peak * trail_capture)
        elif time_stop_mult is not None and peak < time_stop_mult:
            # Time-stop: bail near entry when the token never performs.
            exit_mult = 0.95
        else:
            exit_mult = 1.0 - stop_pct / 100.0
        proceeds += remaining * POSITION * exit_mult
        sells += 1

    return proceeds - POSITION - FEE_TX * (1 + sells)


def evaluate(peaks, name, **kw):
    pnls = [sim(p, **kw) for p in peaks]
    wins = sum(1 for p in pnls if p > 0)
    return (
        name,
        sum(pnls),
        statistics.mean(pnls),
        statistics.median(pnls),
        wins,
        len(pnls),
    )


def main():
    live = real_peaks()
    shadow = shadow_peaks()
    print(f"real live trades: {len(live)}   shadow 0.01 cohort: {len(shadow)}")
    print(f"trail realisation assumption: {TRAIL_REALISATION:.0%} of peak\n")

    ladders = [
        ("A. TP1 1.5x/70% + trail", dict(tp1_x=1.5, tp1_frac=0.70, trail_capture=TRAIL_REALISATION, stop_pct=15)),
        ("B. TP1 1.5x/100% (all out)", dict(tp1_x=1.5, tp1_frac=1.00, trail_capture=0, stop_pct=15)),
        ("C. TP1 1.2x/100%", dict(tp1_x=1.2, tp1_frac=1.00, trail_capture=0, stop_pct=15)),
        ("D. TP1 1.2x/70% + trail", dict(tp1_x=1.2, tp1_frac=0.70, trail_capture=TRAIL_REALISATION, stop_pct=15)),
        ("E. A + time-stop if <1.2x", dict(tp1_x=1.5, tp1_frac=0.70, trail_capture=TRAIL_REALISATION, stop_pct=15, time_stop_mult=1.2)),
        ("F. TP1 1.5x/70% + trail, stop 25%", dict(tp1_x=1.5, tp1_frac=0.70, trail_capture=TRAIL_REALISATION, stop_pct=25)),
        ("G. old 1.1x/40% + 1.5x/40%", dict(tp1_x=1.1, tp1_frac=0.40, trail_capture=0, stop_pct=35)),
    ]

    for label, peaks in (("REAL live peaks", live), ("SHADOW peaks", shadow)):
        print(f"=== {label} (n={len(peaks)}) ===")
        header = f"{'ladder':<38}{'total':>9}{'avg':>9}{'median':>9}{'wins':>8}"
        print(header)
        print("-" * len(header))
        results = []
        for name, kw in ladders:
            r = evaluate(peaks, name, **kw)
            results.append(r)
            print(f"{r[0]:<38}{r[1]:>+9.4f}{r[2]:>+9.5f}{r[3]:>+9.5f}{r[4]:>5}/{r[5]}")
        best = max(results, key=lambda r: r[1])
        print(f"best: {best[0]} ({best[1]:+.4f} SOL, {best[2]:+.5f}/trade)\n")


if __name__ == "__main__":
    main()
