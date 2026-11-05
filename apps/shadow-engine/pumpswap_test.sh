#!/bin/bash
# PumpSwap fallback verification: signal a graduated token -> entry via PumpSwap pool
D=/tmp/telshadow
cd ~/Telsolsniper
MINT_BS58="4M5DceVdRSz6Cxjix5uU7PXFL6vuG2hJhQrydbk1pump"  # graduated: PUMPSWAP
rm -f $D/*.sock $D/shadow.sqlite* $D/signals.sqlite*
env -i PATH=$PATH HOME=$HOME RUST_LOG=info SIGNAL_SOCKET_PATH=$D/signals.sock EVENT_SOCKET_PATH=$D/events.sock SIGNAL_DATABASE_PATH=$D/signals.sqlite ./target/debug/telegram-signal-sniper > $D/intake.log 2>&1 &
INTAKE=$!
sleep 1
env -i PATH=$PATH HOME=$HOME RUST_LOG=info EVENT_SOCKET_PATH=$D/events.sock SHADOW_DATABASE_PATH=$D/shadow.sqlite RPC_URL=https://api.mainnet-beta.solana.com ./target/debug/shadow-engine > $D/shadow.log 2>&1 &
SHADOW=$!
sleep 2
MSG_ID=$((777100 + RANDOM % 1000))
printf '{"source":"soltrenchtrending","messageId":%s,"mint":"%s","ticker":"GRAD","messageText":"smoke","receivedAtUnixMs":1800000000000}\n' "$MSG_ID" "$MINT_BS58" | socat - UNIX-CONNECT:$D/signals.sock
sleep 10
tail -n 4 $D/shadow.log
python3 - <<PYEOF
import sqlite3
c = sqlite3.connect("$D/shadow.sqlite")
print("TRADES:", c.execute("select ticker, round(entry_price_sol,10), round(entry_market_cap_sol,1), token_amount, round(peak_price_sol,10) from shadow_trades").fetchall())
PYEOF
kill $INTAKE $SHADOW 2>/dev/null
