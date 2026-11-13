#!/bin/sh
# Records the catalog preview, asciiverse/saalis.cast, to hub/RECORDING.md:
# 100x30, the game flying itself (--demo) through the first waves round
# Routa station, with a fixed seed so every recording shows the same fight.
# asciinema runs headless, so nothing from this terminal gets in; a
# throwaway HOME and state folder keep the log and saves out of yours.
#
#   cargo build --release && asciiverse/record-preview.sh
set -eu
cd "$(dirname "$0")/.."
out=asciiverse/saalis.cast
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
rm -f "$out"
env -i PATH="$PWD/target/release:/usr/bin:/bin:/snap/bin" HOME="$tmp" \
  XDG_STATE_HOME="$tmp/state" XDG_DATA_HOME="$tmp/data" USER=player LOGNAME=player \
  TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8 \
  asciinema rec --headless --window-size 100x30 --idle-time-limit 2 \
    --capture-env TERM,COLORTERM,LANG --title Saalis \
    --command 'timeout --foreground -s INT 45 saalis --demo --mute --seed 7' "$out"
ls -la "$out"
