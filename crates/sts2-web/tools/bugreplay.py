#!/usr/bin/env python3
"""Replays a pasted bug report (WEB42). Usage: bugreplay.py [--base http://host:port/] [--run] [FILE|-]
Reads the report (or just its `v1:...` code) from FILE or stdin, prints the page address that replays it and the
run's header. With --run it opens that address in the Chrome at $CDP_PORT (default 9467, see realtest.sh) and prints
the state it reaches: decision, floor, HP, hand and enemies, for comparing with the report."""
import json, os, re, sys, urllib.parse

args = sys.argv[1:]
base = "http://127.0.0.1:8765/"
run = "--run" in args
if "--base" in args:
    base = args[args.index("--base") + 1]
rest = [a for i, a in enumerate(args) if not a.startswith("--") and (i == 0 or args[i - 1] != "--base")]
text = sys.stdin.read() if not rest or rest[0] == "-" else open(rest[0]).read()
m = re.search(r"\bv1:\d+:[A-Z0-9_-]+:\d+:[^:\s]*:[pd]*:[0-9a-z,]*", text)
if not m:
    sys.exit("no replay code (v1:...) found")
code = m.group(0)
url = base.rstrip("/") + "/?replay=" + urllib.parse.quote(code, safe=":,+")
_, hero, seed, asc, mods, flags, moves = code.split(":")
print(f"hero {hero} seed {seed} ascension {asc} modifiers {mods or '-'} flags {flags or '-'} moves {len(moves.split(',')) if moves else 0}")
print(url)
if run:
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "realtest"))
    from drv import Page
    p = Page(int(os.environ.get("CDP_PORT", 9467)), 390, 664, True)
    p.open(url)
    p.settle(2.5)
    v = p.js("JSON.stringify((() => { const v = sts2.view(); if (!v) return null; const rows = v.screen.blocks.flatMap((b) => b.List ? b.List.rows : []);"
             " return {decision: v.decision, floor: v.save.floor, hp: v.save.hp + '/' + v.save.max_hp, hand: rows.filter((r) => r.card).map((r) => r.card.name),"
             " enemies: v.screen.blocks.filter((b) => b.Enemy).map((b) => b.Enemy.name + ' ' + b.Enemy.hp)}; })())")
    print(v)
