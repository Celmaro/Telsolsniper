#!/bin/bash
# Exit-rule verification: fake a stale entry, then confirm the poller closes it
D=/tmp/telshadow
cd ~/Telsolsniper
rm -f $D/*.sock $D/shadow.sqlite*
# Seed a stale losing trade (entered 2h ago) and a stale winning trade
python3 - <<PYEOF
import sqlite3, time
c = sqlite3.connect("$D/shadow.sqlite")
c.execute("""CREATE TABLE shadow_trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT, mint TEXT NOT NULL, ticker TEXT,
    source TEXT NOT NULL, message_id INTEGER NOT NULL,
    entry_time_unix_ms INTEGER NOT NULL, entry_price_sol REAL NOT NULL,
    entry_market_cap_sol REAL NOT NULL, buy_sol_amount REAL NOT NULL,
    token_amount REAL NOT NULL, curve_complete INTEGER NOT NULL DEFAULT 0,
    exit_time_unix_ms INTEGER, exit_price_sol REAL, exit_reason TEXT,
    pnl_sol REAL, pnl_pct REAL, peak_price_sol REAL,
    UNIQUE (mint, entry_time_unix_ms))""")
now = int(time.time()*1000)
c.execute("INSERT INTO shadow_trades (mint,ticker,source,message_id,entry_time_unix_ms,entry_price_sol,entry_market_cap_sol,buy_sol_amount,token_amount,peak_price_sol) VALUES ('DJhsdZkkw8M7AWoH2u7tZfXxbTN1w3EyWJ39D3gHpump','LOSE','test',1,?,?,730.0,0.5,684155.0,?)", (now-7200000, 7.309563625465203e-07, 7.309563625465203e-07))
c.execute("INSERT INTO shadow_trades (mint,ticker,source,message_id,entry_time_unix_ms,entry_price_sol,entry_market_cap_sol,buy_sol_amount,token_amount,peak_price_sol) VALUES ('DJhsdZkkw8M7AWoH2u7tZfXxbTN1w3EyWJ39D3gHpump','TPTEST','test',3,?,?,730.0,0.5,684155.0,?)", (now-30000, 7.309563625465203e-07/3, 7.309563625465203e-07))
c.execute("INSERT INTO shadow_trades (mint,ticker,source,message_id,entry_time_unix_ms,entry_price_sol,entry_market_cap_sol,buy_sol_amount,token_amount,peak_price_sol) VALUES ('DJhsdZkkw8M7AWoH2u7tZfXxbTN1w3EyWJ39D3gHpump','MAXHOLD','test',2,?,?,730.0,0.5,684155.0,?)", (now-7200001, 7.309563625465203e-07, 7.309563625465203e-07*0.5))
c.commit()
PYEOF
env -i PATH=$PATH HOME=$HOME RUST_LOG=info SIGNAL_SOCKET_PATH=$D/signals.sock EVENT_SOCKET_PATH=$D/events.sock SIGNAL_DATABASE_PATH=$D/signals.sqlite ./target/debug/telegram-signal-sniper > $D/intake.log 2>&1 &
INTAKE=$!
sleep 1
env -i PATH=$PATH HOME=$HOME RUST_LOG=info EVENT_SOCKET_PATH=$D/events.sock SHADOW_DATABASE_PATH=$D/shadow.sqlite RPC_URL=https://api.mainnet-beta.solana.com SHADOW_POLL_SECS=3 ./target/debug/shadow-engine > $D/shadow.log 2>&1 &
SHADOW=$!
sleep 10
echo "=== shadow.log ==="
tail -n 8 $D/shadow.log
python3 - <<PYEOF
import sqlite3
c = sqlite3.connect("$D/shadow.sqlite")
print("TRADES:", c.execute("select ticker, exit_reason, round(pnl_pct,2) from shadow_trades").fetchall())
PYEOF
kill $INTAKE $SHADOW 2>/dev/null
