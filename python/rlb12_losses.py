"""The loss table of a bot's lost seeds (RLB12; the RLB9 classes).

DIR holds `fatal.tsv.<variant>` (`sts2-diff fights --only last --every 1` on the lost seeds,
the bot's own search `h300`, more worlds `w256W64` and the clairvoyant `c`), `sampled*/runs.tsv`
(`run-collect` with the run policy sampled) and `clair/rows.tsv` (`bot --clairvoyant-fights`).
Classes: (a) search blunder, the fatal start is won by more worlds or the clairvoyant search;
(c) weak deck or HP, not (a) but a re-run of the seed wins; (e) unwinnable, nothing wins.

    python3 python/rlb12_losses.py DIR      # writes DIR/losses.tsv, prints the table
"""

import collections
import glob
import pathlib
import sys


def fatal(path):
    return {r[0]: r for r in (line.split("\t") for line in open(path).read().splitlines())}


def main():
    d = pathlib.Path(sys.argv[1])
    base, more, clair = (fatal(d / f"fatal.tsv.{v}") for v in ("h300", "w256W64", "c"))
    sampled = collections.Counter()
    n_sampled = 0
    for path in glob.glob(str(d / "sampled*/runs.tsv")):
        n_sampled += 1
        for line in open(path):
            r = line.split("\t")
            sampled[r[1]] += r[4] == "1"
    clair_run = set()
    if (d / "clair/rows.tsv").exists():
        for line in open(d / "clair/rows.tsv"):
            r = line.split("\t")
            if r[1] == "victory":
                clair_run.add(r[0].removeprefix("RUN"))
    rows, by_class, by_fight = [], collections.Counter(), collections.defaultdict(collections.Counter)
    for seed, r in base.items():
        a = more[seed][3] == "1" or clair[seed][3] == "1"
        c = sampled[seed] > 0 or seed in clair_run
        cls = "a" if a else "c" if c else "e"
        by_class[cls] += 1
        by_fight[r[2]][cls] += 1
        rows.append([seed, r[1], r[2], r[4], more[seed][3], clair[seed][3], str(sampled[seed]),
                     str(int(seed in clair_run)), cls])
    with open(d / "losses.tsv", "w") as f:
        f.write("seed\tkind\tfight\thp_in\tw256_won\tclair_won\tsampled_wins\tclair_run_won\tclass\n")
        for row in rows:
            f.write("\t".join(row) + "\n")
    won_any = sum(1 for s in base if sampled[s] > 0)
    rate = sum(sampled[s] for s in base) / max(1, n_sampled * len(base))
    print(f"lost {len(base)}: (a) {by_class['a']} (c) {by_class['c']} (e) {by_class['e']}; "
          f"{n_sampled} sampled re-runs win {rate:.3f} of them, {won_any} won by one")
    for fight, c in sorted(by_fight.items(), key=lambda x: -sum(x[1].values())):
        print(f"  {fight:28} {sum(c.values()):3}  a {c['a']:3} c {c['c']:3} e {c['e']:3}")


if __name__ == "__main__":
    main()
