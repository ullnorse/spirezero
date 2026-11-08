"""RLB16: per-phase stats from `fights --trace` files (incoming attack damage, block, HP lost,
damage dealt). usage: rlb16_tracestats.py LABEL TRACE..."""
import re, sys, glob, collections
ENEMY = re.compile(r"\[(\d+) (-?\d+)/(\d+) b(\d+) ([^<]*)<([^>]*)>\]")
def parse(path):
    turns = []; cur = None
    for line in open(path):
        m = re.match(r"(start )?T(\d+) hp (-?\d+) blk (\d+)", line)
        if not m: continue
        en = [(int(a), int(h), int(mx), int(b), p, i) for a, h, mx, b, p, i in ENEMY.findall(line)]
        rec = dict(turn=int(m.group(2)), hp=int(m.group(3)), blk=int(m.group(4)), en=en)
        if m.group(1): cur = {"start": rec}; turns.append(cur)
        elif cur is not None and "end" not in cur: cur["end"] = rec
    return turns
def incoming(en):
    tot = 0
    for *_, intents in en:
        for it in intents.split(","):
            m = re.match(r"\s*(Attack|DeathBlow) (\d+)(?:x(\d+))?", it)
            if m: tot += int(m.group(2)) * int(m.group(3) or 1)
    return tot
def phase(name, en):
    if "Queen" in name:
        return "amalgam alive" if any(mx == 199 and h > 0 for _, h, mx, *_ in en) else "queen alone"
    mx = max((m for _, h, m, *_ in en), default=0)
    return {100: "form 1", 200: "form 2", 300: "form 3"}.get(mx, "?")
stats = collections.defaultdict(lambda: collections.Counter())
for path in sys.argv[2:]:
    name = path.split("/")[-1]; T = parse(path)
    for a, b in zip(T, T[1:] + [None]):
        if "end" not in a: continue
        ph = phase(name, a["start"]["en"]); st = stats[(sys.argv[1], "Queen" if "Queen" in name else "TestSubject", ph)]
        inc = incoming(a["end"]["en"]); blk = a["end"]["blk"]
        hp_next = b["start"]["hp"] if b else 0
        lost = a["end"]["hp"] - hp_next
        st["turns"] += 1; st["incoming"] += inc; st["block"] += min(blk, inc); st["hp_lost"] += max(lost, 0)
        st["dealt"] += sum(h for _, h, *_ in a["start"]["en"]) - sum(h for _, h, *_ in a["end"]["en"])
        st["unblocked>=20"] += inc - blk >= 20
for k, st in sorted(stats.items()):
    t = st["turns"]
    print("%-6s %-12s %-14s turns %4d  incoming/t %5.1f  blocked %4.0f%%  hp lost/t %5.1f  dealt/t %5.1f  big hits %4.0f%%" % (
        *k, t, st["incoming"] / t, 100 * st["block"] / max(st["incoming"], 1), st["hp_lost"] / t, st["dealt"] / t, 100 * st["unblocked>=20"] / t))
