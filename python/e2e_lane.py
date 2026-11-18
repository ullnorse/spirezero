"""The NECRO1 training lane (`docs/plans/necro-e2e.md` §7 WP7): `train_e2e.Learner` run under a
watchdog. Started by `tools/e2e_lane.sh` as the `necro-e2e` systemd user unit; the learner's
maths is untouched.

    python e2e_lane.py run --out DIR [--set key=json ...] [--pin-at 100e6,250e6,...] [--seed-range A,B]
    python e2e_lane.py status --out DIR

Files in DIR (default `~/spirebot/target/rl-checkpoints/NECRO1`):
`state.pt` every `ckpt_every` (5M) decisions and at every exit; `pinned/state-<tag>.pt` plus
`pinned/MANIFEST.tsv` (sha256) at 100M, 250M, 500M, 1B and every 500M after; `stats.jsonl`
per iteration; `lane.log` (every watchdog action); `PAUSE` (touch it: the trainer finishes its
iteration, saves, frees the GPU and waits until it is gone, then resumes from `state.pt`);
`STOPPED` (the watchdog's reason for a final stop; delete it to allow `start`).

**Watchdog** (all in this process; systemd's `Restart=on-failure` does the restarting, and an
exit code of 75 asks for it):
- every start is logged; the 5th start inside 2 h, counted since the last manual start, stops
  the lane (`STOPPED`);
- free disk under 30 GB stops it; so do two non-finite iterations within 100M decisions (the
  first one exits 75 without saving, so the restart resumes from the last good `state.pt`);
- a rate under half its median (of the last 200 iterations) for 10 minutes, paused time
  excluded, saves and exits 75; an iteration that has run over 10 minutes and over twice the
  median's duration (a hang) exits 75 without saving (a heartbeat thread);
- every pinned checkpoint is checked with `python -m sts2.e2e.honesty` (H2-H4, CPU and CUDA);
  a failure stops the lane.

`--pin-bench` (RLN43, off by default): after a pin passes honesty, the combat gym's bench
(`rln43_gym.py bench`: the human panel and the RLN39 own-entry panel) runs on it as its own
systemd user unit `<out dir, lower case>-bench-<tag>` (`necro5-bench-500M`) on the CPU (4 threads, Nice 19), writing
`pinned/gym-<tag>.json` and `.log`; the lane does not wait for it, and its result never stops
the lane.
"""

import argparse
import dataclasses
import datetime
import hashlib
import json
import os
import pathlib
import shutil
import signal
import statistics
import subprocess
import sys
import threading
import time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
# Before torch loads (it reads this at import): see train_e2e.
os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

DEFAULT_OUT = pathlib.Path("~/spirebot/target/rl-checkpoints/NECRO1").expanduser()
GATES = [100e6, 250e6, 500e6, 1000e6]
GATE_EVERY = 500e6
MIN_FREE_GB = float(os.environ.get("LANE_MIN_FREE_GB", 30))
START_LIMIT = (4, 2 * 3600)  # more than 4 starts in 2 h
NAN_WINDOW = 100_000_000
SLOW_FRACTION, SLOW_SECONDS = 0.5, 600
RESTART = 75  # the exit status that asks systemd for a restart


def now_iso():
    return datetime.datetime.now().astimezone().isoformat(timespec="seconds")


def log(out, msg):
    line = f"{time.time():.0f} {now_iso()} {msg}"
    with open(out / "lane.log", "a") as f:
        f.write(line + "\n")
    print(line, flush=True)


def gate_tag(g):
    return f"{g / 1e9:g}B" if g >= 1e9 else f"{g / 1e6:g}M"


def gate_after(done_tags, override=None):
    """The first gate (decisions) whose tag is not pinned yet."""
    if override:
        return next((g for g in override if gate_tag(g) not in done_tags), None)
    g = 0.0
    gates = list(GATES)
    while True:
        g = gates.pop(0) if gates else g + GATE_EVERY
        if gate_tag(g) not in done_tags:
            return g


def read_manifest(out):
    path = out / "pinned" / "MANIFEST.tsv"
    if not path.exists():
        return []
    rows = [ln.split("\t") for ln in path.read_text().splitlines()[1:] if ln]
    return rows


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 22), b""):
            h.update(chunk)
    return h.hexdigest()


def starts_in_window(out, window):
    path = out / "lane.log"
    if not path.exists():
        return 0
    cutoff = time.time() - window
    n = 0
    for ln in path.read_text().splitlines():
        p = ln.split(" ", 2)
        if len(p) != 3 or float(p[0]) < cutoff:
            continue
        if p[2].startswith("MANUAL START"):  # `e2e_lane.sh start`: a person restarted it
            n = 0
        elif p[2].startswith("START "):
            n += 1
    return n


def stop_lane(out, reason):
    (out / "STOPPED").write_text(f"{now_iso()} {reason}\n")
    log(out, f"STOP {reason}")


def recent_rates(out, k=200):
    path = out / "stats.jsonl"
    if not path.exists():
        return []
    lines = path.read_text().splitlines()[-k:]
    return [json.loads(ln)["total_dps"] for ln in lines if ln]


class Lane:
    def __init__(self, args):
        import torch
        import train_e2e as T

        self.torch, self.T = torch, T
        self.args = args
        self.out = pathlib.Path(args.out).expanduser()
        self.out.mkdir(parents=True, exist_ok=True)
        self.pause_file = self.out / "PAUSE"
        self.term = False
        self.nosave = False
        self.paused = False
        self.last_progress = time.monotonic()
        self.rates = recent_rates(self.out)
        self.slow_since = None
        override = [float(x) for x in args.pin_at.split(",")] if args.pin_at else None
        self.override = override
        self.pinned_tags = {r[0] for r in read_manifest(self.out)}

    # ---- helpers ----

    def config(self):
        T = self.T
        state = self.out / "state.pt"
        if state.exists():
            cfg = T.Config.from_dict(self.torch.load(state, weights_only=False)["config"])
            for kv in self.args.set:  # a resume takes speed flags and practice, as train_e2e.py
                k, v = kv.split("=", 1)
                if k in T.RESUME_SETTABLE:
                    T.set_resumed(cfg, k, json.loads(v))
                else:
                    log(self.out, f"ignored --set {k} on resume (learning settings stay)")
            return cfg, True
        over = {}
        for kv in self.args.set:
            k, v = kv.split("=", 1)
            over[k] = json.loads(v)
        if self.args.seed_range:
            over["seed_range"] = [int(x) for x in self.args.seed_range.split(",")]
        cfg = T.Config.from_dict({**dataclasses.asdict(T.Config()), **over})
        (self.out / "config.json").write_text(json.dumps(dataclasses.asdict(cfg), indent=1))
        return cfg, False

    def build(self, cfg, resume):
        return self.T.Learner(cfg, self.out, resume=resume)

    def heartbeat(self):
        while True:
            time.sleep(30)
            if self.paused:
                continue
            med = statistics.median(self.rates) if len(self.rates) >= 30 else None
            if med is None:
                continue
            rows = self.cfg.envs * self.cfg.steps
            expect = rows / med
            elapsed = time.monotonic() - self.last_progress
            if elapsed > SLOW_SECONDS and elapsed > 2 * expect:
                log(self.out, f"WATCHDOG hang: no iteration for {elapsed:.0f}s "
                              f"(median iteration {expect:.0f}s): exit {RESTART}")
                os._exit(RESTART)

    def free_gb(self):
        return shutil.disk_usage(self.out).free / 2**30

    def pause_cycle(self, learner, cfg):
        log(self.out, f"PAUSE at {learner.decisions} decisions: saving, freeing the GPU")
        learner.save()
        learner.close()
        del learner
        import gc
        gc.collect()
        self.torch.cuda.empty_cache()
        self.paused = True
        while self.pause_file.exists() and not self.term:
            time.sleep(5)
        log(self.out, "RESUME: PAUSE removed, resuming from state.pt")
        learner = self.build(cfg, True)
        self.paused = False
        self.last_progress = time.monotonic()
        self.slow_since = None
        return learner

    def pin(self, learner, gate):
        tag = gate_tag(gate)
        pdir = self.out / "pinned"
        pdir.mkdir(exist_ok=True)
        path = pdir / f"state-{tag}.pt"
        learner.save()
        learner.save(path)
        digest = sha256(path)
        man = pdir / "MANIFEST.tsv"
        if not man.exists():
            man.write_text("tag\tdecisions\tsha256\tbytes\ttime\n")
        with open(man, "a") as f:
            f.write(f"{tag}\t{learner.decisions}\t{digest}\t{path.stat().st_size}\t{now_iso()}\n")
        self.pinned_tags.add(tag)
        log(self.out, f"PIN {tag} at {learner.decisions}: {path} sha256 {digest[:16]}")
        res = subprocess.run([sys.executable, "-m", "sts2.e2e.honesty", str(path)],
                             cwd=HERE, capture_output=True, text=True, timeout=1500)
        report = res.stdout.strip().splitlines()[-1] if res.stdout.strip() else res.stderr[-2000:]
        (pdir / f"honesty-{tag}.json").write_text(report + "\n")
        try:
            verdict = json.loads(report).get("ok")
        except (json.JSONDecodeError, AttributeError):
            verdict = None
        if res.returncode != 0 and verdict is None:
            # The check itself broke (no verdict): training goes on; `status` shows it.
            log(self.out, f"HONESTY ERROR at {tag}: no verdict (see pinned/honesty-{tag}.json); "
                          "run `python -m sts2.e2e.honesty` on the pin by hand")
            return True
        if res.returncode != 0:
            stop_lane(self.out, f"honesty gate FAILED at pinned {tag} (see pinned/honesty-{tag}.json)")
            return False
        log(self.out, f"HONESTY {tag} H2-H4 passed")
        if getattr(self.args, "pin_bench", False):
            self.start_bench(path, tag)
        return True

    def start_bench(self, path, tag):
        """RLN43 `--pin-bench`: the gym bench on pin `path` as a detached CPU unit."""
        pdir = path.parent
        unit = f"{self.out.name.lower()}-bench-{tag}"
        cmd = ["systemd-run", "--user", f"--unit={unit}", "--collect", "-p", "Nice=19",
               "-p", "MemoryMax=12G", f"--working-directory={HERE}",
               "-E", "CUDA_VISIBLE_DEVICES=", "-E", "STS2_THREADS=4", "-E", "OMP_NUM_THREADS=4",
               *(["-E", f"{k}={os.environ[k]}"] for k in ("STS2_FFI_LIB", "STS2_FFI_BUILD_ID",
                                                          "PYTHONPATH") if k in os.environ),
               "sh", "-c", f'"{sys.executable}" rln43_gym.py bench "{path}" '
               f'--out "{pdir}/gym-{tag}.json" > "{pdir}/gym-{tag}.log" 2>&1']
        cmd = [x for part in cmd for x in (part if isinstance(part, list) else [part])]
        try:
            res = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
            ok = res.returncode == 0
        except (OSError, subprocess.SubprocessError):
            ok = False
        log(self.out, f"PIN BENCH {tag}: " + (f"started {unit}" if ok else "could not start"))

    # ---- the loop ----

    def run(self):
        out, args = self.out, self.args
        if (out / "STOPPED").exists():
            log(out, "REFUSED: STOPPED exists: " + (out / "STOPPED").read_text().strip())
            return 0
        n = starts_in_window(out, START_LIMIT[1]) + 1
        log(out, f"START pid={os.getpid()} ({n} in the last 2h)")
        if n > START_LIMIT[0]:
            stop_lane(out, f"{n} starts within 2 h")
            return 0
        cfg, resume = self.config()
        self.cfg = cfg
        signal.signal(signal.SIGTERM, lambda *_: setattr(self, "term", True))
        signal.signal(signal.SIGINT, lambda *_: setattr(self, "term", True))
        learner = self.build(cfg, resume)
        log(out, f"{'resumed' if resume else 'new run'} at {learner.decisions} decisions, "
                 f"{cfg.envs} envs x {cfg.threads} threads, seeds {cfg.seed_range}")
        threading.Thread(target=self.heartbeat, daemon=True).start()
        last_ckpt = learner.decisions
        self.last_progress = time.monotonic()
        try:
            while True:
                if self.term:
                    log(out, "SIGTERM: saving and stopping")
                    break
                if self.pause_file.exists():
                    learner = self.pause_cycle(learner, cfg)
                    last_ckpt = learner.decisions
                    if self.term:
                        continue
                if self.free_gb() < MIN_FREE_GB:
                    stop_lane(out, f"free disk {self.free_gb():.1f} GB < {MIN_FREE_GB} GB")
                    break
                t0 = time.monotonic()
                rec = learner.iterate()
                dur = time.monotonic() - t0
                self.last_progress = time.monotonic()
                self.print_line(rec)
                if not rec["finite"]:
                    path = out / "nan.json"
                    seen = json.loads(path.read_text()) if path.exists() else []
                    seen.append(learner.decisions)
                    path.write_text(json.dumps(seen))
                    if len(seen) >= 2 and seen[-1] - seen[-2] <= NAN_WINDOW:
                        stop_lane(out, f"non-finite twice within 100M ({seen[-2]}, {seen[-1]})")
                    else:
                        log(out, f"NaN at {learner.decisions}: exit {RESTART} without "
                                 "saving, to resume from the last good state.pt")
                        self.nosave = True
                        return RESTART
                    self.nosave = True
                    return 0
                self.rates.append(rec["total_dps"])
                del self.rates[:-200]
                if len(self.rates) >= 30:
                    med = statistics.median(self.rates[:-1])
                    if rec["total_dps"] < SLOW_FRACTION * med:
                        self.slow_since = self.slow_since or (time.monotonic() - dur)
                        if time.monotonic() - self.slow_since >= SLOW_SECONDS:
                            log(out, f"WATCHDOG slow: {rec['total_dps']:.0f}/s under half the "
                                     f"median {med:.0f}/s for 10 min: save, exit {RESTART}")
                            learner.save()
                            learner.close()
                            return RESTART
                    else:
                        self.slow_since = None
                if learner.decisions - last_ckpt >= cfg.ckpt_every:
                    learner.save()
                    last_ckpt = learner.decisions
                gate = gate_after(self.pinned_tags, self.override)
                if gate is not None and learner.decisions >= gate:
                    if not self.pin(learner, gate):
                        break
                    last_ckpt = learner.decisions
        finally:
            if not self.nosave:
                try:
                    learner.save()
                    import numpy as np
                    np.save(out / "floors.npy", np.asarray(learner.floors, dtype=np.int16))
                except Exception as e:  # noqa: BLE001
                    log(out, f"final save failed: {e!r}")
            learner.close()
        return 0

    @staticmethod
    def print_line(rec):
        print(f"it {rec['iteration']:5d} dec {rec['decisions'] / 1e6:8.2f}M "
              f"floor {rec['mean_floor'] or 0:5.2f} win {rec['win_rate'] or 0:.3f} "
              f"dps {rec['total_dps']:6.0f} kl {rec['kl']:.4f} "
              f"klb {rec.get('kl_batch', float('nan')):.4f} "
              f"ent {rec['ent_combat']:.3f}/{rec['ent_run']:.3f}", flush=True)


def status(out, override=None):
    out = pathlib.Path(out).expanduser()
    path = out / "stats.jsonl"
    if not path.exists():
        print("no stats yet")
        return
    recs = [json.loads(ln) for ln in path.read_text().splitlines()[-100:] if ln]
    last = recs[-1]
    tail = recs[-10:]
    rate = statistics.mean(r["total_dps"] for r in tail)
    floors = [r["mean_floor"] for r in recs[-50:] if r["mean_floor"] is not None]
    wins = [r["win_rate"] for r in recs[-50:] if r["win_rate"] is not None]
    man = read_manifest(out)
    gate = gate_after({r[0] for r in man}, override)
    eta = (gate - last["decisions"]) / rate if gate else float("nan")
    ckpt = out / "state.pt"
    state_age = (time.time() - ckpt.stat().st_mtime) / 60 if ckpt.exists() else float("nan")
    print(f"decisions   {last['decisions'] / 1e6:.1f}M (iteration {last['iteration']})")
    print(f"rate        {rate:.0f} decisions/s (last 10 iterations)")
    print(f"floor       last {last['mean_floor'] or 0:.2f}   mean(50 it) "
          f"{statistics.mean(floors) if floors else 0:.2f}")
    print(f"win rate    last {last['win_rate'] or 0:.4f}   mean(50 it) "
          f"{statistics.mean(wins) if wins else 0:.4f}")
    print(f"entropy     combat {last['ent_combat']:.3f}   run {last['ent_run']:.3f}")
    print(f"kl          {last['kl']:.4f}   batch {last.get('kl_batch', float('nan')):.4f}   lr factor {last['lr_factor']:.2f}")
    print(f"checkpoint  state.pt {state_age:.0f} min ago; pinned: "
          f"{', '.join(r[0] for r in man) or 'none'}")
    print(f"paused      {'YES (PAUSE file)' if (out / 'PAUSE').exists() else 'no'}")
    if (out / "STOPPED").exists():
        print("STOPPED     " + (out / "STOPPED").read_text().strip())
    print(f"next gate   {gate_tag(gate) if gate else '-'}, ETA {eta / 3600:.1f} h"
          if gate else "next gate   -")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("cmd", choices=("run", "status"))
    ap.add_argument("--out", default=str(DEFAULT_OUT))
    ap.add_argument("--set", action="append", default=[], help="key=json Config overrides")
    ap.add_argument("--seed-range", default=None, help="A,B (a smoke test's throwaway seeds)")
    ap.add_argument("--pin-at", default=None, help="comma list of decisions (a smoke test)")
    ap.add_argument("--pin-bench", action="store_true", help="RLN43: the gym bench on each pin")
    args = ap.parse_args(argv)
    if args.cmd == "status":
        status(args.out, [float(x) for x in args.pin_at.split(",")] if args.pin_at else None)
        return 0
    return Lane(args).run()


if __name__ == "__main__":
    sys.exit(main())
