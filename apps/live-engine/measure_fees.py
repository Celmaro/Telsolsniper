#!/usr/bin/env python3
"""Measure real on-chain CU and fee paid for our recent trades."""
import json
import sqlite3
import statistics
import urllib.request

RPC = "https://api.mainnet-beta.solana.com"
DB = "/home/agentuser/Telsolsniper/data/live.sqlite"


def rpc(method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(RPC, data=body, headers={"Content-Type": "application/json"})
    return json.load(urllib.request.urlopen(req, timeout=30))


def main():
    db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
    db.row_factory = sqlite3.Row
    sigs = list(
        db.execute(
            "SELECT side, kind, signature FROM order_journal "
            "WHERE state='confirmed' AND signature IS NOT NULL ORDER BY id DESC LIMIT 30"
        )
    )
    buys, sells = [], []
    for s in sigs:
        try:
            res = rpc("getTransaction", [s["signature"], {"encoding": "jsonParsed", "maxSupportedTransactionVersion": 0}]).get("result")
        except Exception:
            continue
        if not res or not res.get("meta"):
            continue
        meta = res["meta"]
        cu = meta.get("computeUnitsConsumed") or 0
        fee = meta.get("fee") or 0
        keys = res["transaction"]["message"]["accountKeys"]
        signers = sum(1 for k in keys if isinstance(k, dict) and k.get("signer"))
        base = 5000 * max(signers, 1)
        rec = {"cu": cu, "fee": fee, "prio": fee - base}
        (buys if s["side"] == "buy" else sells).append(rec)

    for name, rows in (("BUY", buys), ("SELL", sells)):
        if not rows:
            continue
        cus = sorted(r["cu"] for r in rows)
        fees = sorted(r["fee"] for r in rows)
        prios = sorted(r["prio"] for r in rows)
        print(f"{name} n={len(rows)}")
        print(f"  CU consumed : median {statistics.median(cus):,.0f}  p95 {cus[int(len(cus)*0.95)-1]:,}  max {max(cus):,}")
        print(f"  fee paid    : median {statistics.median(fees):,.0f}  max {max(fees):,} lamports")
        print(f"  priority    : median {statistics.median(prios):,.0f}  max {max(prios):,} lamports")
        implied = [r["prio"] / max(r["cu"], 1) * 1e6 for r in rows if r["cu"]]
        if implied:
            print(f"  implied bid : median {statistics.median(implied):,.0f} uLamports/CU")

    print()
    print("configured: cu_limit=500,000  base bid=150,000  cap=2,000,000  tip=20,000 lamports")
    if buys:
        med_cu = statistics.median(r["cu"] for r in buys)
        print(f"buy CU headroom: limit 500,000 vs median use {med_cu:,.0f} -> {500_000/max(med_cu,1):.1f}x overshoot")
        med_prio = statistics.median(r["prio"] for r in buys)
        if med_cu:
            implied_bid = med_prio / med_cu * 1e6
            at_limit = 500_000 * implied_bid / 1e6
            print(f"priority paid {med_prio:,.0f} vs {at_limit:,.0f} if limit were {med_cu*1.25:,.0f} (same bid) "
                  f"-> saves {med_prio - med_cu*1.25*implied_bid/1e6:,.0f} lamports/tx")


if __name__ == "__main__":
    main()
