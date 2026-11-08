#!/usr/bin/env python3
"""Stress test for the referee's `reset` (tracer/Reset.cs): a reused game must give the same traces as fresh ones.

    referee/stress_reset.py [-n JOBS] [-j PARALLEL] [--orders K] [--seed S] [--keep]

Draws N random jobs: forced fights (every committed loadout, so every character, on the encounters it names; plain
encounters with a random character; a tenth with a loadout that opens a choice from the turn loop) and whole runs
(random/greedy/shopper policy, optionally a run loadout and the real-game branches), at ascension 0 or 10, with a
random action budget; the engine (`sts2-diff gen` / `run-gen`) writes the actions and half the lists are cut at a
random point. Each job runs once in its own fresh `referee.py serve` (the baseline). From each fresh trace, prefix
jobs are cut just after an action that opened a prompt (card or bundle selection, reward, event, shop, rest site) and
after a random action, so that a run stops with something open; their expected trace is the prefix of the fresh one.
Then K warm `serve` processes each play every job in their own shuffled order, ~10% repeated, with
`REFEREE_NO_RETRY=1` so a fresh replay cannot hide a leak. Every warm trace and status must equal the expected one
byte for byte; the differences are listed (with the job before) and the exit status is 1 if there is any. The trace
store is bypassed. Work files are in target/stress_reset/ (deleted on success unless --keep).
"""

import argparse
import concurrent.futures
import json
import os
import random
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REFEREE = ROOT / "referee" / "referee.py"
BIN = ROOT / "target" / "debug" / "sts2-diff"
CHARACTERS = ["Necrobinder", "Ironclad", "Silent", "Defect", "Regent"]
# What opens a card choice from a turn-start or turn-end hook, inside the combat's turn loop (the T14 leak; found in the
# decomp as the models with such a hook that call CardSelectCmd). A tenth of the jobs use a loadout with one.
TURN_PROMPT_IDS = re.compile(r'"(FOREGONE_CONCLUSION|ENTROPY|TYRANNY|TOOLS_OF_THE_TRADE|GAMBLING_CHIP|TOOLBOX|'
                             r'TOASTY_MITTENS|CHOICES_PARADOX)[+":*]')
# Decisions that leave something open when a run stops on them.
PROMPTS = {"card_select", "bundle_select", "card_reward", "event_choice", "shop", "rest_site"}


def plan():
    """(encounter jobs, loadout jobs, run loadout files) from tools/regress_plan.py."""
    out = subprocess.run([sys.executable, "-I", str(ROOT / "tools" / "regress_plan.py"), str(ROOT), "1", "x", "200", "0"],
                         capture_output=True, text=True, check=True).stdout
    encounters, loadouts = [], []
    for line in out.splitlines():
        _, _, encounter, loadout, *_ = line.split()  # label asc encounter loadout key character act1
        encounter = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", encounter).upper()  # as tools/diff.sh
        if loadout == "-":
            encounters.append(encounter)
        else:
            loadouts.append((encounter, str(ROOT / loadout)))
    runs = sorted(str(p) for p in (ROOT / "crates" / "sts2-diff" / "run_loadouts").glob("*.json"))
    return sorted(set(encounters)), loadouts, runs


def gen(job, work, rng):
    """Writes the job's actions with the engine; False if the engine cannot play it."""
    i, seed = job["i"], job["seed"]
    actions, trace = work / f"{i}.actions.json", work / f"{i}.rust.jsonl"
    asc = str(rng.choice([0, 10]))
    if job["mode"] == "combat":
        cmd = [BIN, "gen", "--seed", seed, "--policy-seed", str(rng.randrange(1 << 30)),
               "--max-actions", str(rng.randint(3, 120)), "--encounter", job["encounter"], "--ascension", asc]
        if job["loadout"]:
            cmd += ["--loadout", job["loadout"]]
        else:
            cmd += ["--character", job["character"]]
    else:
        cmd = [BIN, "run-gen", "--seed", seed, "--policy-seed", str(rng.randrange(1 << 30)),
               "--max-actions", str(rng.randint(10, 700)), "--policy", job["policy"], "--ascension", asc]
        if job["run_loadout"]:
            cmd += ["--run-loadout", job["run_loadout"]]
        if job["real"]:
            cmd += ["--real-game", "1"]
    cmd += ["--actions", actions, "--trace", trace]
    ok = subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0 and actions.exists()
    trace.unlink(missing_ok=True)
    if ok and rng.random() < 0.5:
        # Most plays run to the end of the fight; a prefix stops anywhere (mid-turn, on a prompt the turn loop or an
        # enemy turn opened, mid-choice), which is what a reset must clean up.
        spec = json.loads(actions.read_text())
        spec["actions"] = spec["actions"][:rng.randint(0, len(spec["actions"]))]
        actions.write_text(json.dumps(spec))
    job["actions"] = str(actions)
    job["desc"] = " ".join(str(c) for c in cmd[1:-4]).replace(str(ROOT) + "/", "")
    return ok


def serve(lines, env):
    """Runs `<seed>\t<actions>\t<trace>` lines through one `referee.py serve`; its status lines."""
    r = subprocess.run([sys.executable, "-I", str(REFEREE), "serve"], input="".join(lines), capture_output=True,
                       text=True, env=env)
    status = r.stdout.splitlines()
    if len(status) != len(lines):
        sys.exit(f"serve answered {len(status)} of {len(lines)} jobs: {r.stderr[-2000:]}")
    return status


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("-n", type=int, default=300)
    ap.add_argument("-j", type=int, default=4, help="fresh processes in parallel")
    ap.add_argument("--orders", type=int, default=2, help="warm processes, each over every job in its own order")
    ap.add_argument("--seed", type=int, default=random.randrange(1 << 30))
    ap.add_argument("--keep", action="store_true")
    args = ap.parse_args()
    rng = random.Random(args.seed)
    print(f"stress_reset: seed {args.seed}, {args.n} jobs, {args.orders} warm orders", flush=True)

    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(ROOT / "Cargo.toml"), "-p", "sts2-diff"], check=True)
    env = dict(os.environ, REFEREE_NO_TRACE_CACHE="1", REFEREE_NO_RETRY="1")  # the raw reset, no fresh replay
    env.pop("REFEREE_STATS", None)
    env["STS2_REFEREE"] = subprocess.run([sys.executable, "-I", str(REFEREE), "where"], capture_output=True, text=True,
                                         check=True, env=env).stdout.strip()  # one build for the whole test
    print(f"referee build {env['STS2_REFEREE']}", flush=True)
    work = ROOT / "target" / "stress_reset"
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)

    encounters, loadouts, run_loadouts = plan()
    jobs = []
    while len(jobs) < args.n:
        i = len(jobs)
        kind = rng.random()
        if kind < 0.1:
            encounter, loadout = rng.choice([j for j in loadouts if TURN_PROMPT_IDS.search(Path(j[1]).read_text())])
            job = {"mode": "combat", "encounter": encounter, "loadout": loadout}
        elif kind < 0.45:
            encounter, loadout = rng.choice(loadouts)
            job = {"mode": "combat", "encounter": encounter, "loadout": loadout}
        elif kind < 0.7:
            job = {"mode": "combat", "encounter": rng.choice(encounters), "loadout": None,
                   "character": rng.choice(CHARACTERS)}
        else:
            policy = rng.choice(["random", "greedy", "shopper"])
            job = {"mode": "run", "policy": policy, "run_loadout": rng.choice(run_loadouts) if rng.random() < 0.4 else None,
                   "real": policy == "shopper" and rng.random() < 0.5}
        job.update(i=i, seed=f"SR{rng.randrange(10**6)}")
        if gen(job, work, rng):
            jobs.append(job)
    modes = {}
    for job in jobs:
        spec = json.loads(Path(job["actions"]).read_text())
        key = f"{job['mode']}/{spec.get('character', 'Necrobinder')}"
        modes[key] = modes.get(key, 0) + 1
    print("jobs: " + ", ".join(f"{k} {v}" for k, v in sorted(modes.items())), flush=True)

    def fresh(job):
        return serve([f"{job['seed']}\t{job['actions']}\t{work / str(job['i'])}.fresh.jsonl\n"], env)[0]

    with concurrent.futures.ThreadPoolExecutor(args.j) as pool:
        baseline = list(pool.map(fresh, jobs))
    for job, status in zip(jobs, baseline):
        job["status"] = status
        job["want"] = (work / f"{job['i']}.fresh.jsonl").read_bytes()
    failed = sum(s != "ok" for s in baseline)

    # Prefix jobs: a job cut just after an action that opened a prompt (or after a random action), so that its run
    # ends on that prompt. The fresh trace of a prefix is the prefix of the fresh trace (a run is deterministic), so
    # these need no fresh run of their own.
    prefixes = []
    for job in jobs:
        lines = job["want"].splitlines(keepends=True)
        steps = [n for n, line in enumerate(lines) if n and json.loads(line)["decision"] in PROMPTS]
        if job["status"] != "ok":
            lines = lines[:-1]  # the failing step and after are not a valid prefix
            steps = [n for n in steps if n < len(lines)]
        cuts = {rng.choice(steps)} if steps else set()
        if len(lines) > 1:
            cuts.add(rng.randrange(1, len(lines)))
        spec = json.loads(Path(job["actions"]).read_text())
        for cut in sorted(cuts):
            i = len(jobs) + len(prefixes)
            actions = work / f"{i}.actions.json"
            actions.write_text(json.dumps({**spec, "actions": spec["actions"][:cut]}))
            prefixes.append({"i": i, "seed": job["seed"], "actions": str(actions), "status": "ok",
                             "want": b"".join(lines[:cut + 1]), "desc": f"{job['desc']} [:{cut}]",
                             "prompt": json.loads(lines[cut])["decision"]})
    jobs += prefixes
    on_prompt = sum(p["prompt"] in PROMPTS for p in prefixes)
    print(f"fresh: {len(baseline)} runs, {failed} failed (the same failure is expected warm); "
          f"{len(prefixes)} prefix jobs, {on_prompt} ending on a prompt", flush=True)

    def warm(k):
        order = list(range(len(jobs))) + rng.sample(range(len(jobs)), len(jobs) // 10)
        random.Random(args.seed + k).shuffle(order)
        lines = [f"{jobs[i]['seed']}\t{jobs[i]['actions']}\t{work / str(i)}.warm{k}.{n}.jsonl\n" for n, i in enumerate(order)]
        return order, serve(lines, env)

    with concurrent.futures.ThreadPoolExecutor(args.orders) as pool:
        results = list(pool.map(warm, range(args.orders)))
    diffs = []
    for k, (order, status) in enumerate(results):
        for n, i in enumerate(order):
            job = jobs[i]
            got = (work / f"{i}.warm{k}.{n}.jsonl").read_bytes()
            if status[n] != job["status"] or got != job["want"]:
                prev = jobs[order[n - 1]]["desc"] if n else "-"
                diffs.append(f"order {k} #{n} job {i} ({job['desc']}), after ({prev}): "
                             f"fresh '{job['status'][:120]}', warm '{status[n][:120]}'"
                             + ("" if got == job["want"] else ", traces differ"))
    total = sum(len(o) for o, _ in results)
    for d in diffs:
        print(d)
    print(f"stress_reset: {total} warm runs of {len(jobs)} jobs, {len(diffs)} differ from fresh (seed {args.seed})")
    if not diffs and not args.keep:
        shutil.rmtree(work)
    sys.exit(1 if diffs else 0)


if __name__ == "__main__":
    main()
