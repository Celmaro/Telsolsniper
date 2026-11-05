#!/bin/bash
# PumpSwap exit verification: seed a stale entry on the graduated token, poller must close it via sell math
D=/tmp/telshadow
cd ~/Telsolsniper
rm -f $D/*.sock $D/shadow.sqlite*
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
# Entered 2h ago at 1/3 current price -> take_profit should fire via sell math
c.execute("INSERT INTO shadow_trades (mint,ticker,source,message_id,entry_time_unix_ms,entry_price_sol,entry_market_cap_sol,buy_sol_amount,token_amount,peak_price_sol) VALUES ('4M5DceVdRSz6Cxjix5uU7PXFL6vuG2hJhQrydbk1pump','SWAPTP','test',1,?,?,3455.0,0.5,30850.512672,?)", (now-7200000, 1.6018338583563443e-05/3, 1.6018338583563443e-05))
c.commit()
PYEOF
env -i PATH=$PATH HOME=$HOME RUST_LOG=info SIGNAL_SOCKET_PATH=$D/signals.sock EVENT_SOCKET_PATH=$D/events.sock SIGNAL_DATABASE_PATH=$D/signals.sqlite ./target/debug/telegram-signal-sniper > $D/intake.log 2>&1 &
INTAKE=$!
sleep 1
env -i PATH=$PATH HOME=$HOME RUST_LOG=info EVENT_SOCKET_PATH=$D/events.sock SHADOW_DATABASE_PATH=$D/shadow.sqlite RPC_URL=https://api.mainnet-beta.solana.com SHADOW_POLL_SECS=3 ./target/debug/shadow-engine > $D/shadow.log 2>&1 &
SHADOW=$!
sleep 10
tail -n 3 $D/shadow.log
python3 - <<PYEOF
import sqlite3
c = sqlite3.connect("$D/shadow.sqlite")
print("TRADES:", c.execute("select ticker, exit_reason, round(pnl_pct,2), round(pnl_sol,4) from shadow_trades").fetchall())
PYEOF
kill $INTAKE $SHADOW 2>/dev/null
