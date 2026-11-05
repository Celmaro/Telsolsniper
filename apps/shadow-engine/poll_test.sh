#!/bin/bash
# Polling/exit verification with a fresh mint each run
D=/tmp/telshadow
cd ~/Telsolsniper
MINT=$(curl -s --max-time 10 "https://api.dexscreener.com/token-boosts/latest/v1" | python3 -c "
import json,sys
d=json.load(sys.stdin)
for x in d:
    if x.get('chainId')=='solana' and x.get('url','').startswith('https://dexscreener.com/solana/'):
        u=x['url'].split('/')[-1]
        if '-' not in u: print(u); break
")
MINT=$(curl -s --max-time 10 "https://api.dexscreener.com/token-boosts/latest/v1" | python3 -c "
import json,sys
d=json.load(sys.stdin)
print(' '.join(x['tokenAddress'] for x in d if x.get('chainId')=='solana'))")
# pick first mint with an active pump.fun curve
MINT_BS58=""
for m in $MINT; do
  OUT=$(cargo run -q -p shadow-engine --example probe -- "$m" 2>/dev/null | tail -n 1)
  case "$OUT" in
    *"complete false"*) MINT_BS58="$m"; echo "picked: $OUT"; break;;
  esac
done
if [ -z "$MINT_BS58" ]; then echo "no active-curve mint found"; exit 1; fi
MSG_ID=$((999200 + RANDOM % 1000))
rm -f $D/*.sock $D/shadow.sqlite*
env -i PATH=$PATH HOME=$HOME RUST_LOG=info $(grep -v '^$' $D/intake.env | tr '\n' ' ') ./target/debug/telegram-signal-sniper > $D/intake.log 2>&1 &
INTAKE=$!
sleep 1
env -i PATH=$PATH HOME=$HOME RUST_LOG=info $(grep -v '^$' $D/shadow.env | tr '\n' ' ') ./target/debug/shadow-engine > $D/shadow.log 2>&1 &
SHADOW=$!
sleep 2
printf '{"source":"soltrenchtrending","messageId":%s,"mint":"%s","ticker":"PROBE","messageText":"smoke","receivedAtUnixMs":1800000000000}\n' "$MSG_ID" "$MINT_BS58" | socat - UNIX-CONNECT:$D/signals.sock
sleep 20
echo "=== shadow.log ==="
tail -5 $D/shadow.log
python3 - <<PYEOF
import sqlite3
c = sqlite3.connect("$D/shadow.sqlite")
print("TRADES:", c.execute("select ticker, round(entry_price_sol,12), round(exit_price_sol,12), exit_reason, round(pnl_pct,2), round(peak_price_sol,12) from shadow_trades").fetchall())
PYEOF
kill $INTAKE $SHADOW 2>/dev/null
