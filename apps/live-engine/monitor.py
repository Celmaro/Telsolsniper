#!/usr/bin/env python3
"""Live-engine monitor: prints ONLY when something changed.

Watchdog pattern for the 5-minute cron: empty stdout = nothing to report.
Reports new entries/fills, failures in the journal, and wallet balance drop.
State in data/.live-monitor-state.json.
"""
import json
import sqlite3
import subprocess
import sys
from pathlib import Path

ROOT = Path("/home/agentuser/Telsolsniper")
DB = ROOT / "data" / "live.sqlite"
STATE = ROOT / "data" / ".live-monitor-state.json"


def load_state():
    if STATE.exists():
        try:
            return json.loads(STATE.read_text())
        except Exception:
            pass
    return {"last_fill_id": 0, "last_trade_id": 0, "reported_errors": []}


def save_state(state):
    STATE.write_text(json.dumps(state))


def main():
    if not DB.exists():
        return  # nothing to report yet
    state = load_state()
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    lines = []

    # New trades since last check
    trades = db.execute(
        "SELECT id, mint, ticker, source, entry_price_sol, buy_sol_amount, token_amount,"
        " entry_signature, exit_time_unix_ms, exit_reason, pnl_sol, tp1_done, tp2_done"
        " FROM live_trades WHERE id > ? ORDER BY id",
        (state["last_trade_id"],),
    ).fetchall()
    for t in trades:
        if t["exit_time_unix_ms"] is None:
            lines.append(
                f"BUY {t['ticker'] or t['mint'][:8]} {t['mint'][:8]}… "
                f"@ {t['entry_price_sol']:.3e} SOL ({t['buy_sol_amount']} SOL) sig={t['entry_signature']}"
            )
        else:
            pnl = t["pnl_sol"] or 0
            lines.append(
                f"EXIT {t['ticker'] or t['mint'][:8]} {t['mint'][:8]}… "
                f"{t['exit_reason']} pnl={pnl:+.5f} SOL"
            )
        state["last_trade_id"] = max(state["last_trade_id"], t["id"])

    # New fills (TP stages)
    fills = db.execute(
        "SELECT id, trade_id, kind, multiple, tokens_sold, proceeds_sol, signature"
        " FROM live_fills WHERE id > ? ORDER BY id",
        (state["last_fill_id"],),
    ).fetchall()
    for f in fills:
        lines.append(
            f"FILL trade#{f['trade_id']} {f['kind']} {f['multiple']:.2f}x "
            f"sold={f['tokens_sold']:.0f} got={f['proceeds_sol']:.6f} SOL sig={f['signature']}"
        )
        state["last_fill_id"] = max(state["last_fill_id"], f["id"])

    # Engine errors since last check
    try:
        out = subprocess.run(
            ["journalctl", "--user", "-u", "telsolsniper-live", "--since", "6 min ago", "--no-pager"],
            capture_output=True, text=True, timeout=20,
        ).stdout
        for line in out.splitlines():
            low = line.lower()
            if any(k in low for k in ("buy failed", "sell failed", "error", "panic", "insufficient")):
                key = line[-80:]
                if key not in state["reported_errors"]:
                    state["reported_errors"].append(key)
                    lines.append(f"ENGINE: {line.strip()[:220]}")
    except Exception:
        pass
    state["reported_errors"] = state["reported_errors"][-50:]

    # Open positions summary
    row = db.execute(
        "SELECT COUNT(*) n, COALESCE(SUM(realized_pnl_sol),0) pnl FROM live_trades"
        " WHERE exit_time_unix_ms IS NULL"
    ).fetchone()
    total = db.execute(
        "SELECT COUNT(*) n, COALESCE(SUM(pnl_sol),0) pnl FROM live_trades"
    ).fetchone()
    if lines:
        lines.append(f"open={row['n']} open_pnl={row['pnl']:+.5f} SOL | total_trades={total['n']} realized={total['pnl']:+.5f} SOL")

    save_state(state)
    if lines:
        text = "\n".join(lines)
        # Durable record so the history survives outside cron output.
        try:
            from datetime import datetime, timezone
            with open(ROOT / "data" / "live-monitor.log", "a") as fh:
                fh.write(f"\n[{datetime.now(timezone.utc):%Y-%m-%d %H:%M:%S}Z]\n{text}\n")
        except Exception:
            pass
        print(text)


if __name__ == "__main__":
    sys.exit(main() or 0)
