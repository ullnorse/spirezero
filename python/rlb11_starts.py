"""Shards the bot's fight starts (`sts2-diff collect-starts`) for `train_combat_puffer.py`.

Runs (by seed) are split: the last `--eval-runs` are held out and written unweighted by fight
kind (`eval/common.jsonl`, `elite.jsonl`, `boss.jsonl`); the rest go, in seed order, into shards
of `--runs-per-shard` runs with elite starts repeated `--elite` times and boss starts `--boss`
times (an env draws starts uniformly from its shard).

    .venv/bin/python python/rlb11_starts.py IN.jsonl [IN2.jsonl ...] --out DIR
"""

import argparse
import collections
import json
import pathlib


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("inputs", nargs="+")
    p.add_argument("--out", required=True)
    p.add_argument("--eval-runs", type=int, default=150)
    p.add_argument("--runs-per-shard", type=int, default=250)
    p.add_argument("--elite", type=int, default=3)
    p.add_argument("--boss", type=int, default=6)
    args = p.parse_args()
    runs = collections.defaultdict(list)
    for path in args.inputs:
        for line in open(path):
            try:
                runs[json.loads(line)["seed"]].append(line.strip())
            except (json.JSONDecodeError, KeyError):
                pass  # a blank line, or a line a collection still writing has cut short
    seeds = sorted(runs, key=lambda s: int(s.removeprefix("RUN")))
    held, train = seeds[-args.eval_runs:], seeds[: -args.eval_runs]
    out = pathlib.Path(args.out)
    (out / "eval").mkdir(parents=True, exist_ok=True)
    kinds = {"common": [], "elite": [], "boss": []}
    for s in held:
        for line in runs[s]:
            r = json.loads(line)
            kinds["boss" if r["boss"] else "elite" if r["big"] else "common"].append(line)
    for k, lines in kinds.items():
        (out / "eval" / f"{k}.jsonl").write_text("\n".join(lines) + "\n")
    for i in range(0, len(train), args.runs_per_shard):
        lines = []
        for s in train[i: i + args.runs_per_shard]:
            for line in runs[s]:
                r = json.loads(line)
                lines += [line] * (args.boss if r["boss"] else args.elite if r["big"] else 1)
        (out / f"shard-{i // args.runs_per_shard:03d}.jsonl").write_text("\n".join(lines) + "\n")
    print(f"{len(train)} train runs, {len(held)} held out:",
          {k: len(v) for k, v in kinds.items()})


if __name__ == "__main__":
    main()
