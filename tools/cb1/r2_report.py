"""CB1 round 2: deaths and the four mistakes per run (cb1_mcts.py run .npz files), on all
fights and on the lost ones, per turn end (potions: per potion).

    python tools/cb1/r2_report.py A.npz B.npz ...
"""
import json
import pathlib
import sys

import numpy as np


def main():
    print(f"{'run':34s} {'n':>5s} {'boss1':>6s} {'boss2':>6s} {'boss3':>6s} {'boss':>6s} {'elite':>6s}"
          f" | lost: {'unused':>6s} {'underb':>6s} {'osty':>6s} {'potE':>6s} | all: {'unused':>6s}"
          f" {'underb':>6s} ms/dec")
    for f in sys.argv[1:]:
        z = np.load(f)
        d, room, act = z["died"].astype(bool), z["room"], z["act"]

        def pct(sel):
            return f"{100 * d[sel].mean():6.1f}" if sel.any() else f"{'-':>6s}"
        boss = room == "boss"
        cells = [pct(boss & (act == a)) for a in range(3)] + [pct(boss), pct(room == "elite")]

        def rate(key, sel, den="ends"):
            if f"mis_{key}" not in z.files:
                return f"{'-':>6s}"
            return f"{z[f'mis_{key}'][sel].sum() / max(1, z[f'mis_{den}'][sel].sum()):6.3f}"
        lost = [rate("unused", d), rate("underblock", d), rate("osty_low", d),
                rate("pot_early", d, "potions")]
        alls = [rate("unused", ~d | d), rate("underblock", ~d | d)]
        meta = json.loads(str(z["meta"])) if "meta" in z.files else {}
        st = meta.get("stats", {})
        ms = 1000 * meta.get("wall_s", 0) / max(1, st.get("decisions", 1)) if not meta.get("greedy") else 0
        print(f"{pathlib.Path(f).stem[:34]:34s} {len(d):5d} " + " ".join(cells) + " | "
              + " ".join(lost) + " | " + " ".join(alls) + f" {ms:6.1f}")


if __name__ == "__main__":
    main()
