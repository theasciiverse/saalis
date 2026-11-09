#!/usr/bin/env python3
# Walks @ beside a glyph on a saalis deck running under tools/run.sh,
# by BFS over the tmux screen: tools/walk.py i
import subprocess, sys, collections, time
T=["tmux","-L","saalis"]
def screen():
    return subprocess.run(T+["capture-pane","-t","s","-p"],capture_output=True,text=True).stdout.split("\n")
def find(rows,ch,x0=36):
    alone=lambda r,x: not (x>0 and r[x-1].isalpha()) and not (x+1<len(r) and r[x+1].isalpha())
    return [(x,y) for y,r in enumerate(rows) for x,c in enumerate(r) if c==ch and x>=x0 and 4<y<len(rows)-4 and alone(r,x)]
target=sys.argv[1]
for step in range(200):
    rows=screen()
    me=find(rows,'@')
    goal=find(rows,target)
    if not me or not goal: print("lost", me, goal); sys.exit(1)
    me=me[0]; g=goal[0]
    if max(abs(me[0]-g[0]),abs(me[1]-g[1]))<=1: print("beside", target); sys.exit(0)
    ok=lambda x,y: 0<=y<len(rows) and 0<=x<len(rows[y]) and rows[y][x] in "·+"
    prev={me:None}; q=collections.deque([me]); end=None
    while q:
        p=q.popleft()
        if max(abs(p[0]-g[0]),abs(p[1]-g[1]))<=1: end=p; break
        for d in [(0,-1),(1,0),(0,1),(-1,0)]:
            n=(p[0]+d[0],p[1]+d[1])
            if n not in prev and ok(*n): prev[n]=p; q.append(n)
    if end is None: print("no path"); sys.exit(1)
    while prev[end]!=me: end=prev[end]
    key={(0,-1):'k',(1,0):'l',(0,1):'j',(-1,0):'h'}[(end[0]-me[0],end[1]-me[1])]
    subprocess.run(T+["send-keys","-t","s",key]); time.sleep(0.05)
print("too far"); sys.exit(1)

