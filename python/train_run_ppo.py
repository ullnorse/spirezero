"""The learned run policy: behaviour cloning from the hand rules, then PPO on whole A0 Ironclad
runs (`docs/specs/run-policy.md`; task RLB8).

The rollouts are Rust: `sts2-diff run-collect` plays train seeds on 16 threads with the run
decisions by the hand rules (BC) or the current export (PPO, sampled), the fights by the cheap
honest search, and writes each run decision's features (`crates/sts2-diff/src/honest/policy.rs`).
This script trains `RunNet` on them and writes the export the next collection and
`sts2-diff bot --run-policy` load.

    cargo build --release -p sts2-diff
    .venv/bin/python python/train_run_ppo.py --out target/rlb8/ppo

Return of a run: `win + 0.5 * floor / 49` as potential shaping on the total floor, gamma 1.
"""

import argparse
import os
import json
import pathlib
import struct
import subprocess
import time

import numpy as np
import torch
import torch.nn as nn

MAGIC = b"STS2RUN1"
BIN = os.environ.get("STS2_DIFF_BIN", "target/release/sts2-diff")
TRAIN_FIRST, TRAIN_COUNT = 1_000_000, 100_000
DEV_FIRST = 8_000_000
FLOOR_WEIGHT = 0.5 / 49
MAX_RSS_GB = 12
MAX_COLLECT_SECS = 1200
SETTINGS = {}


def collect(out, first, count, policy=None, greedy=False, rng=0, search=(4, 4, 16, 12), jobs=16):
    search, jobs = SETTINGS.get("search", search), SETTINGS.get("jobs", jobs)
    """Runs `run-collect`; the summary text."""
    w, bw, bossw, beam = search
    cmd = [BIN, "run-collect", "--first", str(first), "--count", str(count), "--out", str(out),
           "--rng", str(rng), "--jobs", str(jobs), "--worlds", str(w), "--big-worlds", str(bw),
           "--boss-worlds", str(bossw), "--beam", str(beam)]
    cmd += SETTINGS.get("fight_args", [])
    if policy is not None:
        cmd += ["--policy", str(policy)]
    if greedy:
        cmd += ["--greedy"]
    with open(pathlib.Path(out).parent / f"{pathlib.Path(out).name}.stderr", "w") as err:
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=err, text=True)
        started = time.time()
        # A watchdog: a run that loops can grow without bound (`docs/specs/run-policy.md`).
        while True:
            try:
                stdout, _ = proc.communicate(timeout=5)
                break
            except subprocess.TimeoutExpired:
                rss = rss_gb(proc.pid)
                limit = SETTINGS.get("max_collect_secs", MAX_COLLECT_SECS)
                if rss > MAX_RSS_GB or time.time() - started > limit:
                    proc.kill()
                    proc.communicate()
                    progress = pathlib.Path(out) / "progress.tsv"
                    if progress.exists():
                        progress.rename(pathlib.Path(out).parent / f"killed-{int(time.time())}.tsv")
                    raise RuntimeError(f"collection killed: rss {rss:.1f} GB, "
                                       f"{time.time() - started:.0f}s")
    if proc.returncode != 0:
        raise RuntimeError(f"collection failed with status {proc.returncode}")
    return stdout


def rss_gb(pid):
    try:
        for line in open(f"/proc/{pid}/status"):
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) / 2**20
    except OSError:
        pass
    return 0.0


def load(path):
    """A collection's arrays as numpy."""
    path = pathlib.Path(path)
    meta = json.loads((path / "meta.json").read_text())
    dec = np.fromfile(path / "dec.bin", dtype="<u4").reshape(-1, 6)
    n = len(dec)
    data = {
        "meta": meta,
        "run": dec[:, 0].astype(np.int64),
        "start": dec[:, 1].astype(np.int64),
        "n": dec[:, 2].astype(np.int64),
        "chosen": dec[:, 3].astype(np.int64),
        "floor": dec[:, 4].astype(np.int64),
        "logp": dec[:, 5].copy().view("<f4"),
        "tok": np.fromfile(path / "dec_tok.bin", dtype="<u2").reshape(n, meta["state_tokens"]),
        "scal": np.fromfile(path / "dec_scal.bin", dtype="<f4").reshape(n, meta["state_scalars"]),
        "ctok": np.fromfile(path / "cand_tok.bin", dtype="<u2").reshape(-1, meta["cand_tokens"]),
        "cscal": np.fromfile(path / "cand_scal.bin", dtype="<f4").reshape(-1, meta["cand_scalars"]),
    }
    runs = [line.split("\t") for line in (path / "runs.tsv").read_text().splitlines()]
    data["run_floor"] = np.array([int(r[3]) for r in runs], dtype=np.int64)
    data["run_won"] = np.array([int(r[4]) for r in runs], dtype=np.int64)
    data["run_end"] = [r[2] for r in runs]
    return data


def concat(parts):
    """Collections joined; candidate starts and run ids shifted."""
    out = {"meta": parts[0]["meta"], "run_end": []}
    cands, runs = 0, 0
    keys = ["run", "start", "n", "chosen", "floor", "logp", "tok", "scal", "ctok", "cscal",
            "run_floor", "run_won"]
    acc = {k: [] for k in keys}
    for p in parts:
        for k in keys:
            v = p[k]
            if k == "start":
                v = v + cands
            if k == "run":
                v = v + runs
            acc[k].append(v)
        cands += len(p["ctok"])
        runs += len(p["run_floor"])
        out["run_end"] += p["run_end"]
    for k in keys:
        out[k] = np.concatenate(acc[k])
    return out


def returns(data, lam=0.95, values=None):
    """Per-decision rewards, returns and (with `values`) GAE advantages, gamma 1."""
    n = len(data["run"])
    reward = np.zeros(n, dtype=np.float32)
    last = np.ones(n, dtype=bool)
    last[:-1] = data["run"][1:] != data["run"][:-1]
    nxt_floor = np.empty(n, dtype=np.int64)
    nxt_floor[:-1] = data["floor"][1:]
    run = data["run"]
    nxt_floor[last] = data["run_floor"][run[last]]
    reward = FLOOR_WEIGHT * (nxt_floor - data["floor"]).astype(np.float32)
    reward[last] += data["run_won"][run[last]].astype(np.float32)
    ret = np.zeros(n, dtype=np.float32)
    adv = np.zeros(n, dtype=np.float32)
    g, a = 0.0, 0.0
    for i in range(n - 1, -1, -1):
        if last[i]:
            g, a = 0.0, 0.0
            v_next = 0.0
        else:
            v_next = values[i + 1] if values is not None else 0.0
        g = reward[i] + g
        ret[i] = g
        if values is not None:
            delta = reward[i] + v_next - values[i]
            a = delta + lam * a
            adv[i] = a
    return ret, adv


class RunNet(nn.Module):
    """The Rust `RunNet`: state MLP, a shared candidate scorer, a value head."""

    def __init__(self, meta, embed=32, hidden=256, second=128):
        super().__init__()
        self.meta = meta
        self.embed_dim, self.hidden, self.second = embed, hidden, second
        self.emb = nn.Embedding(meta["tokens"], embed, padding_idx=0)
        nn.init.normal_(self.emb.weight, std=0.1)
        with torch.no_grad():
            self.emb.weight[0].zero_()
        s = meta["state_scalars"]
        self.l1 = nn.Linear(s + 4 * embed, hidden)
        self.l2 = nn.Linear(hidden, hidden)
        self.v1 = nn.Linear(hidden, second)
        self.v2 = nn.Linear(second, 1)
        self.lh = nn.Linear(hidden, hidden)
        self.lc = nn.Linear(embed + meta["cand_scalars"], hidden, bias=False)
        self.l3 = nn.Linear(hidden, second)
        self.l4 = nn.Linear(second, 1)
        m = meta
        self.bounds = [0, m["deck_tokens"], m["deck_tokens"] + m["relic_tokens"],
                       m["deck_tokens"] + m["relic_tokens"] + m["belt_tokens"], m["state_tokens"]]
        self.scales = [0.1, 0.2, 1.0, 1.0]

    def state(self, tok, scal):
        parts = [scal]
        e = self.emb(tok)
        for k in range(4):
            parts.append(e[:, self.bounds[k]:self.bounds[k + 1]].sum(1) * self.scales[k])
        h = torch.relu(self.l2(torch.relu(self.l1(torch.cat(parts, 1)))))
        return h, self.v2(torch.relu(self.v1(h))).squeeze(1)

    def forward(self, tok, scal, ctok, cscal, seg):
        """Logits per candidate (flat) and values per decision."""
        h, value = self.state(tok, scal)
        hp = self.lh(h)
        c = self.emb(ctok).sum(1)
        z = torch.relu(hp[seg] + self.lc(torch.cat([c, cscal], 1)))
        logits = self.l4(torch.relu(self.l3(z))).squeeze(1)
        return logits, value

    def tensors(self):
        return [self.emb.weight, self.l1.weight, self.l1.bias, self.l2.weight, self.l2.bias,
                self.v1.weight, self.v1.bias, self.v2.weight, self.v2.bias, self.lh.weight,
                self.lh.bias, self.lc.weight, self.l3.weight, self.l3.bias, self.l4.weight,
                self.l4.bias]

    def load_export(self, path):
        """The weights of an `export` file (the inverse of `export`)."""
        m = self.meta
        raw = pathlib.Path(path).read_bytes()
        assert raw[:8] == MAGIC
        head = struct.unpack("<9I", raw[8:44])
        want = (m["feature_version"], m["tokens"], m["state_tokens"], m["state_scalars"],
                m["cand_tokens"], m["cand_scalars"], self.embed_dim, self.hidden, self.second)
        assert head == want, (head, want)
        floats = np.frombuffer(raw, dtype="<f4", offset=52)
        at = 0
        with torch.no_grad():
            for t in self.tensors():
                k = t.numel()
                t.copy_(torch.as_tensor(floats[at:at + k].reshape(t.shape)))
                at += k
        assert at == len(floats), (at, len(floats))

    def export(self, path, vocab_hash):
        m = self.meta
        with open(path, "wb") as f:
            f.write(MAGIC)
            f.write(struct.pack("<9I", m["feature_version"], m["tokens"], m["state_tokens"],
                                m["state_scalars"], m["cand_tokens"], m["cand_scalars"],
                                self.embed_dim, self.hidden, self.second))
            f.write(struct.pack("<Q", vocab_hash))
            for t in self.tensors():
                f.write(t.detach().cpu().numpy().astype("<f4").tobytes())


class Batch:
    """A collection on the GPU, with minibatch gathering of the ragged candidates."""

    def __init__(self, data, device):
        self.d = device
        t = lambda x, dt: torch.as_tensor(x, dtype=dt, device=device)  # noqa: E731
        self.tok = t(data["tok"].astype(np.int32), torch.int32)
        self.scal = t(data["scal"], torch.float32)
        self.ctok = t(data["ctok"].astype(np.int32), torch.int32)
        self.cscal = t(data["cscal"], torch.float32)
        self.start = t(data["start"], torch.int64)
        self.n = t(data["n"], torch.int64)
        self.chosen = t(data["chosen"], torch.int64)
        self.logp = t(data["logp"], torch.float32)
        self.size = len(data["run"])

    def gather(self, idx):
        n = self.n[idx]
        seg = torch.repeat_interleave(torch.arange(len(idx), device=self.d), n)
        first = torch.cumsum(n, 0) - n
        within = torch.arange(int(n.sum()), device=self.d) - first[seg]
        cand = self.start[idx][seg] + within
        return (self.tok[idx].long(), self.scal[idx], self.ctok[cand].long(), self.cscal[cand],
                seg, first + self.chosen[idx])


def segment_stats(logits, seg, groups, chosen):
    """Log-probability of `chosen` (flat), entropy per group."""
    m = torch.full((groups,), -1e30, device=logits.device)
    m = m.scatter_reduce(0, seg, logits, reduce="amax", include_self=True)
    z = torch.exp(logits - m[seg])
    s = torch.zeros(groups, device=logits.device).index_add_(0, seg, z)
    lse = torch.log(s) + m
    logp_all = logits - lse[seg]
    p = torch.exp(logp_all)
    ent = -torch.zeros(groups, device=logits.device).index_add_(0, seg, p * logp_all)
    return logp_all[chosen], ent


def values_of(net, batch, bs=8192):
    out = []
    with torch.no_grad():
        for i in range(0, batch.size, bs):
            idx = torch.arange(i, min(i + bs, batch.size), device=batch.d)
            _, v = net.state(batch.tok[idx].long(), batch.scal[idx])
            out.append(v)
    return torch.cat(out).cpu().numpy()


def logp_of(net, batch, bs=8192):
    out = []
    with torch.no_grad():
        for i in range(0, batch.size, bs):
            idx = torch.arange(i, min(i + bs, batch.size), device=batch.d)
            tok, scal, ctok, cscal, seg, ch = batch.gather(idx)
            logits, _ = net(tok, scal, ctok, cscal, seg)
            lp, _ = segment_stats(logits, seg, len(idx), ch)
            out.append(lp)
    return torch.cat(out).cpu().numpy()


def bc(net, data, device, epochs, lr, log):
    """Cross-entropy on the hand choices and value regression on the returns."""
    batch = Batch(data, device)
    ret = torch.as_tensor(returns(data)[0], device=device)
    opt = torch.optim.Adam(net.parameters(), lr=lr)
    n = batch.size
    split = int(n * 0.95)
    perm = torch.randperm(n, device=device)
    train_idx, val_idx = perm[:split], perm[split:]
    for epoch in range(epochs):
        order = train_idx[torch.randperm(len(train_idx), device=device)]
        tot, cnt = 0.0, 0
        for i in range(0, len(order), 2048):
            idx = order[i:i + 2048]
            tok, scal, ctok, cscal, seg, ch = batch.gather(idx)
            logits, v = net(tok, scal, ctok, cscal, seg)
            lp, _ = segment_stats(logits, seg, len(idx), ch)
            loss = -lp.mean() + 0.5 * ((v - ret[idx]) ** 2).mean()
            opt.zero_grad()
            loss.backward()
            opt.step()
            tot += float(loss) * len(idx)
            cnt += len(idx)
        with torch.no_grad():
            tok, scal, ctok, cscal, seg, ch = batch.gather(val_idx[:20000])
            logits, v = net(tok, scal, ctok, cscal, seg)
            lp, _ = segment_stats(logits, seg, min(len(val_idx), 20000), ch)
            # Agreement: the chosen candidate has the top logit.
            m = torch.full((min(len(val_idx), 20000),), -1e30, device=device)
            m = m.scatter_reduce(0, seg, logits, reduce="amax", include_self=True)
            agree = (logits[ch] >= m).float().mean()
        log(f"bc epoch {epoch} loss {tot / cnt:.4f} val nll {-float(lp.mean()):.4f} "
            f"top1 {float(agree):.3f}")


def ppo_update(net, opt, data, device, args, log):
    batch = Batch(data, device)
    values = values_of(net, batch)
    ret, adv = returns(data, args.lam, values)
    recomputed = logp_of(net, batch)
    drift = float(np.abs(recomputed - data["logp"]).max())
    ret_t = torch.as_tensor(ret, device=device)
    adv_t = torch.as_tensor(adv, device=device)
    adv_t = (adv_t - adv_t.mean()) / (adv_t.std() + 1e-8)
    old_v = torch.as_tensor(values, device=device)
    n = batch.size
    stats = []
    for epoch in range(args.epochs):
        order = torch.randperm(n, device=device)
        kls = []
        for i in range(0, n, args.minibatch):
            idx = order[i:i + args.minibatch]
            tok, scal, ctok, cscal, seg, ch = batch.gather(idx)
            logits, v = net(tok, scal, ctok, cscal, seg)
            lp, ent = segment_stats(logits, seg, len(idx), ch)
            ratio = torch.exp(lp - batch.logp[idx])
            a = adv_t[idx]
            pl = -torch.min(ratio * a, torch.clamp(ratio, 1 - args.clip, 1 + args.clip) * a).mean()
            vc = old_v[idx] + torch.clamp(v - old_v[idx], -0.2, 0.2)
            vl = torch.max((v - ret_t[idx]) ** 2, (vc - ret_t[idx]) ** 2).mean()
            loss = pl + args.vf * vl - args.ent * ent.mean()
            opt.zero_grad()
            loss.backward()
            nn.utils.clip_grad_norm_(net.parameters(), 1.0)
            opt.step()
            kl = float(((ratio - 1) - torch.log(ratio)).mean())
            kls.append(kl)
            stats.append((float(pl), float(vl), float(ent.mean()), kl))
        if np.mean(kls) > args.target_kl:
            break
    s = np.array(stats).mean(0)
    log(f"  ppo pl {s[0]:.4f} vl {s[1]:.4f} ent {s[2]:.3f} kl {s[3]:.4f} epochs {epoch + 1} "
        f"logp drift {drift:.2e} decisions {n}")


def win_rate(summary):
    for line in summary.splitlines():
        if line.startswith("runs "):
            parts = line.split()
            return int(parts[3]), int(parts[1])
    return 0, 0


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--out", default="target/rlb8/ppo")
    p.add_argument("--bc-runs", type=int, default=8192)
    p.add_argument("--bc-epochs", type=int, default=8)
    p.add_argument("--bc-lr", type=float, default=1e-3)
    p.add_argument("--iters", type=int, default=150)
    p.add_argument("--runs", type=int, default=1024)
    p.add_argument("--lr", type=float, default=5e-5)
    p.add_argument("--lr-end", type=float, default=1e-5)
    p.add_argument("--epochs", type=int, default=3)
    p.add_argument("--minibatch", type=int, default=4096)
    p.add_argument("--clip", type=float, default=0.2)
    p.add_argument("--vf", type=float, default=0.5)
    p.add_argument("--ent", type=float, default=0.01)
    p.add_argument("--lam", type=float, default=0.95)
    p.add_argument("--target-kl", type=float, default=0.03)
    p.add_argument("--eval-every", type=int, default=10)
    p.add_argument("--eval-runs", type=int, default=500)
    p.add_argument("--max-hours", type=float, default=6.0)
    p.add_argument("--resume", default=None, help="a checkpoint .pt to start PPO from")
    p.add_argument("--seed-offset", type=int, default=0)
    p.add_argument("--jobs", type=int, default=16)
    p.add_argument("--search", default="4,4,16,12", help="worlds, big worlds, boss worlds, beam")
    p.add_argument("--max-collect-secs", type=int, default=MAX_COLLECT_SECS)
    p.add_argument("--fight-args", default="",
                   help="more run-collect fight flags, e.g. the value search (RLB12): "
                        "'--value builtin --value-leaves --value-hp 300'")
    p.add_argument("--init-bin", default=None,
                   help="an export to warm-start PPO from (fresh optimizer); needs --meta-from")
    p.add_argument("--meta-from", default=None, help="a collection dir whose meta.json matches")
    args = p.parse_args()
    SETTINGS["jobs"] = args.jobs
    SETTINGS["search"] = tuple(int(x) for x in args.search.split(","))
    SETTINGS["fight_args"] = args.fight_args.split()
    SETTINGS["max_collect_secs"] = args.max_collect_secs
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    logf = open(out / "log.txt", "a")
    t0 = time.time()

    def log(msg):
        line = f"[{time.time() - t0:7.0f}s] {msg}"
        print(line, flush=True)
        logf.write(line + "\n")
        logf.flush()

    torch.manual_seed(0)
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    scratch = out / "collect"
    scratch.mkdir(parents=True, exist_ok=True)
    seed_next = TRAIN_FIRST + args.seed_offset
    total_runs = 0

    def next_seeds(count):
        nonlocal seed_next
        if seed_next + count > TRAIN_FIRST + TRAIN_COUNT:
            seed_next = TRAIN_FIRST
        first = seed_next
        seed_next += count
        return first

    if args.init_bin:
        meta = json.loads((pathlib.Path(args.meta_from) / "meta.json").read_text())
        net = RunNet(meta).to(device)
        net.load_export(args.init_bin)
        log(f"warm start from {args.init_bin}")
    elif args.resume:
        saved = torch.load(args.resume, weights_only=False, map_location="cpu")
        net = RunNet(saved["meta"]).to(device)
        net.load_state_dict(saved["model"])
        meta = saved["meta"]
        seed_next = saved.get("seed_next", seed_next)
        total_runs = saved.get("total_runs", 0)
        log(f"resumed {args.resume}")
    else:
        parts = []
        bc_dir = out / "bc"
        for chunk in range(0, args.bc_runs, 2048):
            d = bc_dir / f"{chunk}"
            if not (d / "meta.json").exists():
                s = collect(d, next_seeds(2048), 2048)
                log(f"bc collect {chunk}: {s.splitlines()[0]} | {s.splitlines()[-1]}")
            else:
                next_seeds(2048)
            parts.append(load(d))
        data = concat(parts)
        total_runs += len(data["run_floor"])
        meta = data["meta"]
        log(f"bc data: {len(data['run'])} decisions, {len(data['ctok'])} candidates, "
            f"hand win rate {data['run_won'].mean():.3f}")
        net = RunNet(meta).to(device)
        bc(net, data, device, args.bc_epochs, args.bc_lr, log)
        del data, parts
        torch.save({"model": net.state_dict(), "meta": meta, "seed_next": seed_next,
                    "total_runs": total_runs}, out / "bc.pt")
    vocab_hash = int(meta["vocab_hash"], 16)
    export = out / "policy.bin"
    net.export(export, vocab_hash)
    net.export(out / "bc.bin" if not (args.resume or args.init_bin) else out / "resume.bin",
               vocab_hash)

    def evaluate(tag):
        try:
            s = collect(scratch / "dev", DEV_FIRST, args.eval_runs, export, greedy=True)
        except RuntimeError as e:
            log(f"dev {tag}: {e}")
            return -1
        wins, runs = win_rate(s)
        log(f"dev {tag}: {wins}/{runs} | {s.splitlines()[1]}")
        return wins

    best = evaluate("start")
    net.export(out / "best.bin", vocab_hash)
    opt = torch.optim.Adam(net.parameters(), lr=args.lr, eps=1e-5)
    for it in range(1, args.iters + 1):
        if time.time() - t0 > args.max_hours * 3600:
            log("time limit")
            break
        frac = (it - 1) / max(1, args.iters - 1)
        for g in opt.param_groups:
            g["lr"] = args.lr + (args.lr_end - args.lr) * frac
        for attempt in range(3):
            tc = time.time()
            try:
                s = collect(scratch / "it", next_seeds(args.runs), args.runs, export, rng=it)
                break
            except RuntimeError as e:
                log(f"it {it} attempt {attempt}: {e}; next seeds")
        else:
            raise RuntimeError("three collections failed")
        tc = time.time() - tc
        data = load(scratch / "it")
        total_runs += args.runs
        log(f"it {it} collect {tc:.0f}s ({args.runs / tc * 3600:.0f} runs/h) "
            f"win {data['run_won'].mean():.3f} floor {data['run_floor'].mean():.1f} "
            f"total runs {total_runs}")
        ppo_update(net, opt, data, device, args, log)
        net.export(export, vocab_hash)
        torch.save({"model": net.state_dict(), "meta": meta, "seed_next": seed_next,
                    "total_runs": total_runs}, out / "last.pt")
        if it % args.eval_every == 0:
            wins = evaluate(f"it {it}")
            ckpt = out / f"it{it}"
            net.export(f"{ckpt}.bin", vocab_hash)
            torch.save({"model": net.state_dict(), "meta": meta, "seed_next": seed_next,
                        "total_runs": total_runs}, f"{ckpt}.pt")
            if wins > best:
                best = wins
                net.export(out / "best.bin", vocab_hash)
                log(f"  new best {wins}")
    log(f"done: best dev {best}/{args.eval_runs}, total runs {total_runs}")


if __name__ == "__main__":
    main()
