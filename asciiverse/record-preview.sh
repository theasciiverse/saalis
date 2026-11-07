#!/bin/sh
# Records the catalog preview, asciiverse/saalis.cast, to hub/RECORDING.md:
# 100x30, the game flying itself (--demo) through the first waves round
# Routa station, with a fixed seed so every recording shows the same fight.
# Runs asciinema in a detached tmux pane on a private socket, so nothing
# from this terminal gets in; a throwaway HOME keeps the log out of yours.
#
#   cargo build --release && asciiverse/record-preview.sh
set -eu
cd "$(dirname "$0")/.."
out=asciiverse/saalis.cast
tmp=$(mktemp -d)
trap 'tmux -L saalis-rec kill-server 2>/dev/null || true; rm -rf "$tmp"' EXIT
rm -f "$out"
tmux -L saalis-rec new-session -d -s rec -x 100 -y 30 \
  "env -i PATH=$PWD/target/release:/usr/bin:/bin:/snap/bin HOME=$tmp USER=player LOGNAME=player \
     TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8 \
   asciinema rec --window-size 100x30 --idle-time-limit 2 \
     --capture-env TERM,COLORTERM,LANG --title Saalis \
     --command 'saalis --demo --mute --seed 7' $out"
sleep 45
tmux -L saalis-rec send-keys -t rec C-c
sleep 2
ls -la "$out"
