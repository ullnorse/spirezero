"""RLN42: behaviour telemetry from the training collect, and the NECRO4 behaviour watchdog.

**Telemetry** (`Behaviour`, `train_e2e` `--set behaviour=true`; off = no work, bit-identical
training). Per env, counters over the current episode, from the run decisions the collect samples
(the `rln42_prior.scan` entries) and the actions; added to the collect's totals when a normal run
ends and dropped when a practice episode ends. `stats.jsonl` gets `behaviour` per iteration: the
raw counts of the runs that ended in it (`counts`, so a reader can pool iterations) and the rates:

- `smith`, `smith_hi`: Smith share of rest-site visits that offer Smith and heal (all; HP >=
  70%), the visit's first choice (a return from a cancelled card select is the same visit);
- `skip`: card-reward skip share;
- `removals_per_run`, `removal_share`: shop removals per run; of shop visits whose first row could
  afford the removal, the share that removed;
- `upgrades_per_run`: upgrade levels in the final deck;
- `potions_per_run`: potions used (combat and belt use, not discards); `death_2pot`: share of
  deaths holding >= 2 potions;
- `gold_death`, `floor_death`: mean gold and floor at death; `death_act`: deaths per act 1-3;
- `cancels_per_run`: card selects backed out of (`select_cancel`); `run_rows_per_run`: decisions
  outside combat (NECRO4 at 37M: 47 rest rows per run from a rest -> Smith -> cancel loop).

**Watchdog** (`python rln42_watch.py watch --out DIR`, unit `necro4-watch`): every `every`
decisions (25M), over the latest iterations up to that point with >= `min_runs` finished runs, it
checks the rates against `DIR/watch.json` (written with the defaults below if missing; edit it to
tune, re-read at each check). From `floor.from` on, the rolling mean floor must rise by more than
`floor.min_rise` vs `floor.lag` (50M) earlier; `floor.streak` (2) checks in a row without a rise
fail. A failure touches `DIR/PAUSE` (the lane saves and frees the GPU), writes `DIR/WATCH_ALERT`
(metric, value, threshold, decisions) and logs to `DIR/watch.log` and `DIR/lane.log`; a pass
appends one line to `watch.log`. Checked boundaries are read back from `watch.log`, so a restart
of the watcher does not re-check. It writes nothing else.

**Entropy report** (RLN43, `entropy` in watch.json): from `entropy.from` (10M) on, at each
boundary, the batch entropy fraction per run-decision kind (`train_e2e` `explore`: the
rows-weighted mean over the iterations since the previous boundary, `min_rows` at least) below
its floor is reported, never paused: a `note AT entropy {...}` line in `watch.log` and a
`WATCH NOTE` line in `lane.log`. The floors are the ones `explore_floor` would use (edit them).
"""

import argparse
import json
import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

COUNTS = ("rest", "smith", "rest_hi", "smith_hi", "rewards", "skips", "removals",
          "removal_visits", "potions", "cancels", "run_rows")
C = {k: i for i, k in enumerate(COUNTS)}
TOTALS = COUNTS + ("runs", "wins", "floor_sum", "upgrades", "deaths", "deaths_2pot",
                   "gold_death_sum", "floor_death_sum")
"""The summed counts of a `behaviour` record (and `death_act`, deaths per act 1-3)."""
BELT_AT = 480  # `encode.BELT_AT`: 6 u16 potion ids
GOLD_AT = 12
DEATH_HEADER_ACT = 2
PHASE_COMBAT = 3  # `run_obs.rs` phase code of a fight


class Behaviour:
    """Per-env episode counters (`COUNTS`) and the totals of the runs that ended (module doc)."""

    def __init__(self, n):
        from sts2.e2e import encode as E
        import rln42_prior as RP
        self.RP, self.D = RP, RP.D
        cols = E._table_cols("cpu")
        use = np.zeros(E.ACTIONS_ALL, bool)
        for k in ("combat_potion", "belt_potion"):
            if k in cols:
                use[cols[k].numpy()] = True
        self.potion_use = use
        self.cnt = np.zeros((n, len(COUNTS)), np.int64)
        self.prev = np.full(n, 255, np.int64)  # each env's previous phase
        self.cancel = int(np.flatnonzero(E._action_tables("cpu")[:, 0].numpy()
                                         == E._EK["select_cancel"])[0])
        self.reset_totals()

    def reset_totals(self):
        self.tot = dict({k: 0 for k in TOTALS}, death_act=[0, 0, 0])

    def step(self, sc, actions, phase):
        """One collect step: `sc` the step's entries (`rln42_prior.scan` of every env), `actions`
        (n,) the sampled actions, `phase` (n,) the obs phase byte."""
        cnt, D, kinds = self.cnt, self.D, self.RP.KINDS
        phase = phase.astype(np.int64)
        cnt[:, C["potions"]] += self.potion_use[actions]
        cnt[:, C["cancels"]] += (actions == self.cancel) & (phase == self.RP.P.PHASE_SELECT)
        cnt[:, C["run_rows"]] += phase != PHASE_COMBAT
        revisit = np.isin(self.prev, (self.RP.P.PHASE_SELECT, self.RP.P.PHASE_REST))
        self.prev = phase
        if not len(sc):
            return
        o = sc.obs
        hp = (o[:, 8].astype(np.int64) | o[:, 9].astype(np.int64) << 8) / np.maximum(
            1, o[:, 10].astype(np.int64) | o[:, 11].astype(np.int64) << 8)
        for j in range(len(sc)):
            kind, i = kinds[sc.kind[j]], int(sc.row[j])
            if kind == "shop_remove":  # the removal's card select: a removal was bought
                cnt[i, C["removals"]] += 1
                continue
            if kind == "shop_removal":  # a visit's first row with the removal affordable
                cnt[i, C["removal_visits"]] += 1
                continue
            ch = int(sc.a2o[j, actions[i]])
            if ch < 0:
                continue
            opts = sc.opts[j]
            t = opts[ch][0]
            if kind == "rest" and not revisit[i] and {D.T_SMITH, D.T_HEAL} <= {x[0] for x in opts}:
                cnt[i, C["rest"]] += 1
                cnt[i, C["smith"]] += t == D.T_SMITH
                if hp[j] >= 0.7:
                    cnt[i, C["rest_hi"]] += 1
                    cnt[i, C["smith_hi"]] += t == D.T_SMITH
            elif kind == "card_reward":
                cnt[i, C["rewards"]] += 1
                cnt[i, C["skips"]] += t == D.T_SKIP

    def ended(self, normal, practice, final_obs, floor, won, died, upgrades):
        """Episodes that ended at this step: `normal` (n,) bool runs (counted), `practice` (n,)
        bool practice episodes (dropped); the rest are the env's per-row end arrays."""
        tot = self.tot
        for i in np.nonzero(normal)[0]:
            for k in COUNTS:
                tot[k] += int(self.cnt[i, C[k]])
            tot["runs"] += 1
            tot["wins"] += int(won[i])
            tot["floor_sum"] += int(floor[i])
            tot["upgrades"] += int(upgrades[i])
            if died[i]:
                fo = final_obs[i]
                held = int((fo[BELT_AT:BELT_AT + 12].reshape(6, 2) != 0).any(1).sum())
                tot["deaths"] += 1
                tot["deaths_2pot"] += int(held >= 2)
                tot["gold_death_sum"] += int(fo[GOLD_AT]) | int(fo[GOLD_AT + 1]) << 8
                tot["floor_death_sum"] += int(floor[i])
                tot["death_act"][min(2, int(fo[DEATH_HEADER_ACT]))] += 1
        self.cnt[normal | practice] = 0
        self.prev[normal | practice] = 255

    def record(self):
        """The totals since the last call and their rates (`stats.jsonl` `behaviour`)."""
        t, self.tot = self.tot, None
        self.reset_totals()
        return dict(rates(t), counts=t)


def rates(t):
    """The watched rates of (pooled) `Behaviour` totals; None where nothing was counted."""
    def r(a, b):
        return None if not t.get(b) else round(t[a] / t[b], 4)
    return dict(smith=r("smith", "rest"), smith_hi=r("smith_hi", "rest_hi"),
                skip=r("skips", "rewards"), removals_per_run=r("removals", "runs"),
                removal_share=r("removals", "removal_visits"),
                upgrades_per_run=r("upgrades", "runs"), potions_per_run=r("potions", "runs"),
                death_2pot=r("deaths_2pot", "deaths"), gold_death=r("gold_death_sum", "deaths"),
                floor_death=r("floor_death_sum", "deaths"), floor=r("floor_sum", "runs"),
                cancels_per_run=r("cancels", "runs"), run_rows_per_run=r("run_rows", "runs"))


def pool(recs):
    """Sums the `behaviour` counts of stats records."""
    out = dict({k: 0 for k in TOTALS}, death_act=[0, 0, 0])
    for r in recs:
        b = r["behaviour"]["counts"]
        for k in TOTALS:
            out[k] += b.get(k, 0)
        out["death_act"] = [x + y for x, y in zip(out["death_act"], b["death_act"])]
    return out


# ---------------------------------------------------------------------------------------------
# The watchdog

DEFAULTS = dict(
    every=25_000_000,
    min_runs=2000,
    checks={  # metric: [op, threshold, from decisions]
        "smith_hi": [">=", 0.5, 25_000_000],
        "skip": [">=", 0.10, 25_000_000],
        "removals_per_run": [">=", 0.2, 25_000_000],
        "potions_per_run": [">", 0.5, 25_000_000],
    },
    floor=dict(start=100_000_000, lag=50_000_000, min_rise=0.0, streak=2),
    entropy={"from": 10_000_000, "min_rows": 200,
             "floors": {"rest": 0.15, "card_reward": 0.15, "removal": 0.15}},
)
OPS = {">=": lambda a, b: a >= b, ">": lambda a, b: a > b, "<=": lambda a, b: a <= b,
       "<": lambda a, b: a < b}


def load_stats(path):
    """The log's records by decisions; a resume re-runs the iterations after its state.pt, and
    the later line wins."""
    out = {}
    if path.exists():
        for line in path.read_text().splitlines():
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue  # a line being written
            out[r.get("decisions", 0)] = r
    return [out[k] for k in sorted(out)]


def window(recs, at, min_runs, key=None):
    """The latest records with decisions <= `at` (and `key` present) back to >= `min_runs` runs."""
    sel, runs = [], 0
    for r in reversed(recs):
        if r.get("decisions", 0) > at or (key and "counts" not in r.get(key, {})):
            continue
        sel.append(r)
        runs += r["behaviour"]["counts"]["runs"] if key == "behaviour" else (r.get("runs") or 0)
        if runs >= min_runs:
            return sel, runs
    return None, runs


def mean_floor(recs, at, min_runs):
    sel, runs = window(recs, at, min_runs)
    if sel is None:
        return None
    n = sum(r.get("runs") or 0 for r in sel)
    return sum((r.get("mean_floor") or 0) * (r.get("runs") or 0) for r in sel) / n


def check(recs, at, cfg):
    """The watchdog's verdict at boundary `at`: (failures [(metric, value, threshold)], summary)."""
    fails, summ = [], {}
    sel, runs = window(recs, at, cfg["min_runs"], "behaviour")
    if sel is None:
        summ["behaviour"] = f"only {runs} runs with telemetry"
    else:
        rt = rates(pool(sel))
        summ.update(runs=runs, **{k: rt[k] for k in rt})
        for m, (op, thr, start) in cfg["checks"].items():
            if at < start:
                continue
            v = rt.get(m)
            if v is None or not OPS[op](v, thr):
                fails.append((m, v, f"{op} {thr}"))
    fl = cfg["floor"]
    if at >= fl["start"]:
        rises = []
        for k in range(fl["streak"]):
            b = at - k * cfg["every"]
            now, then = mean_floor(recs, b, cfg["min_runs"]), mean_floor(recs, b - fl["lag"],
                                                                         cfg["min_runs"])
            rises.append(None if now is None or then is None else round(now - then, 3))
        summ["floor_rise"] = rises
        if all(r is not None and r <= fl["min_rise"] for r in rises):
            fails.append(("floor_rise", rises, f"> {fl['min_rise']} vs {fl['lag'] // 10**6}M "
                                                f"earlier, {fl['streak']} checks in a row"))
    return fails, summ


def entropy_low(recs, at, every, cfg):
    """RLN43: {kind: (entropy fraction, rows)} below `cfg["floors"]` over the records in
    (at - every, at], or {} (before `from`, or no `explore` log)."""
    if at < cfg["from"]:
        return {}
    sums = {}
    for r in recs:
        if not at - every < r.get("decisions", 0) <= at:
            continue
        for k, v in ((r.get("explore") or {}).get("kinds") or {}).items():
            if v.get("ent") is not None and v.get("rows"):
                e, n = sums.get(k, (0.0, 0))
                sums[k] = (e + v["ent"] * v["rows"], n + v["rows"])
    out = {}
    for k, floor in cfg["floors"].items():
        e, n = sums.get(k, (0.0, 0))
        if n >= cfg["min_rows"] and e / n < floor:
            out[k] = (round(e / n, 4), n)
    return out


def checked(log):
    out = set()
    if log.exists():
        for line in log.read_text().splitlines():
            if line.startswith("at "):
                out.add(int(line.split()[1]))
    return out


def lane_log(out, msg):
    with open(out / "lane.log", "a") as f:
        f.write(f"{int(time.time())} {time.strftime('%Y-%m-%dT%H:%M:%S%z')} {msg}\n")


def watch_once(out):
    cfg_path = out / "watch.json"
    if not cfg_path.exists():
        cfg_path.write_text(json.dumps(DEFAULTS, indent=1) + "\n")
    cfg = {**DEFAULTS, **json.loads(cfg_path.read_text())}
    recs = load_stats(out / "stats.jsonl")
    if not recs:
        return
    log = out / "watch.log"
    done = checked(log)
    last = recs[-1]["decisions"]
    for at in range(cfg["every"], last + 1, cfg["every"]):
        if at in done or (out / "WATCH_ALERT").exists():
            continue
        fails, summ = check(recs, at, cfg)
        stamp = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        with open(log, "a") as f:
            f.write(f"at {at} {'FAIL' if fails else 'pass'} {stamp} {json.dumps(summ)}"
                    + (f" fails {json.dumps(fails)}" if fails else "") + "\n")
        ent_cfg = {**DEFAULTS["entropy"], **cfg["entropy"]}
        low = entropy_low(recs, at, cfg["every"], ent_cfg)
        if low:  # reported only: no pause
            with open(log, "a") as f:
                f.write(f"note {at} entropy {json.dumps(low)}\n")
            lane_log(out, f"WATCH NOTE at {at / 1e6:.0f}M: entropy below floor "
                     + "; ".join(f"{k}={e} ({n} rows, floor {cfg['entropy']['floors'][k]})"
                                 for k, (e, n) in low.items()))
        if fails:
            (out / "PAUSE").touch()
            (out / "WATCH_ALERT").write_text(json.dumps(dict(
                decisions=at, checked_at_decisions=last, time=stamp,
                failures=[dict(metric=m, value=v, threshold=t) for m, v, t in fails],
                summary=summ), indent=1) + "\n")
            lane_log(out, f"WATCH ALERT at {at / 1e6:.0f}M: "
                     + "; ".join(f"{m}={v} (want {t})" for m, v, t in fails) + " -> PAUSE")
            return


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    w = sub.add_parser("watch")
    w.add_argument("--out", required=True)
    w.add_argument("--every", type=float, default=300, help="seconds between reads")
    w.add_argument("--once", action="store_true")
    a = ap.parse_args()
    out = pathlib.Path(a.out)
    while True:
        watch_once(out)
        if a.once:
            return
        time.sleep(a.every)


if __name__ == "__main__":
    main()
