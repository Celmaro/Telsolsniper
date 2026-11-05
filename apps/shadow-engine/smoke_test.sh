#!/bin/bash
# Smoke test: intake + shadow engine + synthetic signal
set -x
D=/tmp/telshadow
mkdir -p $D
printf 'SIGNAL_SOCKET_PATH=%s/signals.sock\nEVENT_SOCKET_PATH=%s/events.sock\nSIGNAL_DATABASE_PATH=%s/signals.sqlite\n' $D $D $D > $D/intake.env
printf 'EVENT_SOCKET_PATH=%s/events.sock\nSHADOW_DATABASE_PATH=%s/shadow.sqlite\nRPC_URL=https://api.mainnet-beta.solana.com\n' $D $D > $D/shadow.env
rm -f $D/*.sock $D/*.sqlite*
cd ~/Telsolsniper
env -i PATH=$PATH HOME=$HOME RUST_LOG=info $(grep -v '^$' $D/intake.env | tr '\n' ' ') ./target/debug/telegram-signal-sniper > $D/intake.log 2>&1 &
INTAKE_PID=$!
sleep 1
env -i PATH=$PATH HOME=$HOME RUST_LOG=info $(grep -v '^$' $D/shadow.env | tr '\n' ' ') ./target/debug/shadow-engine > $D/shadow.log 2>&1 &
SHADOW_PID=$!
echo "PIDS $INTAKE_PID $SHADOW_PID" > $D/pids
sleep 2
printf '{"source":"soltrenchtrending","messageId":999002,"mint":"HQf2jARrAEr3cZoLTCZgGMY4dUWAraWhsJwaj77FebNJ","ticker":"PROBE","messageText":"smoke","receivedAtUnixMs":1800000000000}\n' | socat - UNIX-CONNECT:$D/signals.sock
sleep 8
tail -3 $D/intake.log
echo ===SHADOW===
tail -6 $D/shadow.log
kill $INTAKE_PID $SHADOW_PID 2>/dev/null
