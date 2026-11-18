"""usage: rows.py RUN.npz...  -- deaths % per run: panel bosses act 1/2/3/all, elites, fights."""
import sys

import numpy as np

for p in sys.argv[1:]:
    z = np.load(p)
    died, room, act = z["died"], z["room"], z["act"]
    boss = room == "boss"
    cells = [died[boss & (act == a)] for a in range(3)] + [died[boss], died[room == "elite"]]
    print(p, *("-" if not len(c) else f"{100 * c.mean():.1f}" for c in cells), len(died),
          sep="\t")
