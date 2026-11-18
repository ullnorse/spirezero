"""CB1: deaths % per act on the human-panel benchmark (`panel_bench.jsonl`: the RLN43 panel
players' boss and elite fight starts, held out of all training), bots next to the humans' own
outcome on the same fights (a different random draw).

    python tools/cb1/panel_report.py A.npz [B.npz ...]
"""
import json
import pathlib
import sys

import numpy as np

recs = {r["id"]: r for r in map(json.loads, open(pathlib.Path(
    "~/.cache/spirebot-data/cb1/panel_bench.jsonl").expanduser()))}
zs = [(pathlib.Path(f).stem, np.load(f)) for f in sys.argv[1:]]
ids = zs[0][1]["ids"]
act = np.array([recs[i]["act"] for i in ids])
room = np.array([recs[i]["room"] for i in ids])
hum = np.array([float(recs[i]["died"]) for i in ids])
print("| room | act | starts | humans | " + " | ".join(n for n, _ in zs) + " |")
print("|---|---|---:|---:|" + "---:|" * len(zs))
for rm in ("boss", "elite"):
    for a in (0, 1, 2, None):
        s = (room == rm) & ((act == a) if a is not None else True)
        cells = []
        for _, z in zs:
            assert (z["ids"] == ids).all()
            d = z["died"][s].astype(float)
            cells.append(f"{100 * d.mean():.1f} ±{100 * d.std() / np.sqrt(d.size):.1f}")
        print(f"| {rm} | {'all' if a is None else a + 1} | {s.sum()} | {100 * hum[s].mean():.1f} | "
              + " | ".join(cells) + " |")
