#!/bin/bash
# Starts saalis in tmux (socket saalis, session s) on scratch data and log
# folders, never the real save: SCRATCH, or target/scratch by default.
# W and H set the size (100x30); args go to the game. Build --release first.
R=$(cd "$(dirname "$0")/.." && pwd)
S=${SCRATCH:-$R/target/scratch}
mkdir -p "$S/data" "$S/slog"
tmux -L saalis kill-server 2>/dev/null
tmux -L saalis new-session -d -s s -x ${W:-100} -y ${H:-30} "SAALIS_DATA_DIR='$S/data' SAALIS_LOG_DIR='$S/slog' '$R/target/release/saalis' --mute $*"
