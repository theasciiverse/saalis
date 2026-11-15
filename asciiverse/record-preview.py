#!/usr/bin/env python3
"""Records the catalog preview, asciiverse/saalis.cast (hub/RECORDING.md),
100x30, about 50 s:

1. A short visit to Routa station on foot (--play --seed 6, docked in the
   hangar): out through the hangar door, along the corridor and into the
   bar, where Ilta Rask tells her story.
2. Then the game flies itself (--demo --seed 6) through a fight round the
   station. Seed 6 never holds one heading for more than a moment: a long
   straight flight at a shallow angle (seed 7's 289 degrees, the old
   preview) shows the ship's glyph stepping a whole cell at a time beside
   its smooth Braille nose line, which reads as a jumping '#'.

asciinema records inside a private tmux server, so the game talks to a real
terminal; this script reads the screen with capture-pane and walks '@' to
Ilta Rask by the shortest open path, step by step, around anyone wandering
in the way. A throwaway HOME and state folder keep saves and logs out of
yours.

    cargo build --release && asciiverse/record-preview.py
"""
import collections, os, shutil, subprocess, sys, tempfile, time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target/release/saalis")
OUT = os.path.join(ROOT, "asciiverse/saalis.cast")
T = ["tmux", "-L", "saalis-rec"]

def screen():
    return subprocess.run(T + ["capture-pane", "-t", "r", "-p"], capture_output=True, text=True).stdout.split("\n")

def key(k, gap=0.15):
    subprocess.run(T + ["send-keys", "-t", "r", k]); time.sleep(gap)

def find(rows, ch):
    # a person is a letter standing on the floor (a floor dot or a door
    # beside them); room names are runs of three or more letters set in a
    # wall. Two people side by side make a run of two, which still counts.
    def run(r, x):
        a = b = x
        while a > 0 and r[a - 1].isalpha(): a -= 1
        while b + 1 < len(r) and r[b + 1].isalpha(): b += 1
        return b - a + 1
    floor = lambda x, y: any(0 <= yy < len(rows) and 0 <= xx < len(rows[yy]) and rows[yy][xx] in "·+"
                             for xx, yy in ((x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)))
    return [(x, y) for y, r in enumerate(rows) for x, c in enumerate(r)
            if c == ch and x > 32 and 4 < y < 26 and floor(x, y) and run(r, x) <= 2]

def walk_to(ch, limit=120):
    for _ in range(limit):
        rows = screen()
        # she may call you over herself as you come into the bar: her talk
        # box covers the deck, and that is the scene we want
        if any("╭ Ilta Rask" in r for r in rows):
            return True
        me, goal = find(rows, "@"), find(rows, ch)
        if not me or not goal:
            if os.environ.get("REC_DEBUG"):
                print("lost", me, goal, file=sys.stderr)
            time.sleep(0.15); continue
        me, g = me[0], goal[0]
        if abs(me[0] - g[0]) + abs(me[1] - g[1]) == 1:
            key({(1, 0): "Right", (-1, 0): "Left", (0, 1): "Down", (0, -1): "Up"}[(g[0] - me[0], g[1] - me[1])])
            return True
        ok = lambda x, y: 0 <= y < len(rows) and 0 <= x < len(rows[y]) and rows[y][x] in "·+"
        prev = {me: None}; q = collections.deque([me]); end = None
        while q:
            p = q.popleft()
            if abs(p[0] - g[0]) + abs(p[1] - g[1]) == 1:
                end = p; break
            for d in [(1, 0), (0, 1), (-1, 0), (0, -1)]:
                n = (p[0] + d[0], p[1] + d[1])
                if n not in prev and ok(*n):
                    prev[n] = p; q.append(n)
        if end is None:
            if os.environ.get("REC_DEBUG"): print("blocked", me, g, file=sys.stderr)
            time.sleep(0.15); continue   # someone in the way: wait a beat
        while prev[end] != me:
            end = prev[end]
        key({(1, 0): "Right", (-1, 0): "Left", (0, 1): "Down", (0, -1): "Up"}[(end[0] - me[0], end[1] - me[1])])
    return False

def main():
    tmp = tempfile.mkdtemp()
    try:
        if os.path.exists(OUT):
            os.remove(OUT)
        env = (f"env -i PATH=/usr/bin:/bin:/snap/bin HOME={tmp} XDG_STATE_HOME={tmp}/state XDG_DATA_HOME={tmp}/data "
               f"SAALIS_LOG_DIR={tmp} USER=player LOGNAME=player TERM=xterm-256color COLORTERM=truecolor LANG=C.UTF-8")
        game = f"sh -c '{BIN} --play --mute --seed 6; exec {BIN} --demo --mute --seed 6'"
        cmd = (f"{env} asciinema rec --overwrite --idle-time-limit 2 --capture-env TERM,COLORTERM,LANG "
               f"--title Saalis --command \"{game}\" {OUT}")
        subprocess.run(T + ["kill-server"], stderr=subprocess.DEVNULL)
        subprocess.run(T + ["new-session", "-d", "-s", "r", "-x", "100", "-y", "30", cmd], check=True)
        time.sleep(3.0)                       # the hangar, a look round
        if not walk_to("i"):
            sys.exit("could not reach Ilta Rask")
        time.sleep(3.5)                       # her first words
        key("Enter", 3.5)                     # and her question
        key("1", 2.5)                         # "I'll get it back."
        for _ in range(4):                    # read through her answer
            if not any("╭ Ilta Rask" in r for r in screen()):
                break
            key("Enter", 1.5)
        key("Escape", 0.6); key("y", 0.5)     # leave; the demo takes over
        time.sleep(32)
        key("q", 2)                           # any key ends the demo
        for _ in range(20):
            if subprocess.run(T + ["has-session", "-t", "r"], stderr=subprocess.DEVNULL).returncode:
                break
            time.sleep(0.5)
    finally:
        subprocess.run(T + ["kill-server"], stderr=subprocess.DEVNULL)
        shutil.rmtree(tmp, ignore_errors=True)
    # the header names the command run; keep the machine's paths out of it
    lines = open(OUT).read().split("\n")
    import json
    h = json.loads(lines[0])
    h["command"] = "saalis --play --seed 6; saalis --demo --seed 6"
    lines[0] = json.dumps(h, ensure_ascii=False)
    open(OUT, "w").write("\n".join(lines))
    print(OUT, os.path.getsize(OUT))

if __name__ == "__main__":
    main()
