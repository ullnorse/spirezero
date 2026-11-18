"""NAZ1: the fight-outcome model (a fight start -> HP-loss distribution and death probability).

    python python/naz1_model.py train --out DIR [--epochs N] ...    # trains on DATA's labelled sets
    python python/naz1_model.py eval DIR                              # the held-out targets
    python python/naz1_model.py speed DIR                             # batched evaluations per second

Use from Python (NAZ2):

    from naz1_model import FightModel
    fm = FightModel.load(DIR, device="cuda")
    out = fm.predict(records)   # records: gym-record dicts (naz1_rollouts module docs)
    out["p_death"], out["mean_loss"], out["hist"]   # numpy; hist over `fm.loss_values` + death

**Input** (one fight start, the gym record's fields): the encounter, the act model, act, floor,
HP, max HP, gold, the potion slots, the deck as (card, upgrade) tokens with their counts, the
relics and the potions. Ascension is A10 (the only one in the data); character Necrobinder.

**Output:** one categorical over `NB_LOSS` survival outcomes (HP lost `LOSS_MIN..LOSS_MAX`, the
ends clipped) plus death. Outcomes a start cannot have (surviving with HP lost >= its HP) are
masked, so the expected HP loss `sum p_b * loss_b + p_death * hp` is consistent with P(death).
Training: cross-entropy against the rollouts' empirical histogram plus a small smooth-L1 term on the
expected loss.

**Net:** a pre-norm transformer over the tokens [CLS, context, encounter, deck..., relics...,
potions...] with a token-type embedding; the context token is an MLP of the scalars. The vocab
(cards, relics, potions, encounters, acts) is built from the training starts and saved with the
weights; an unseen id maps to `<unk>`.
"""

import argparse
import collections
import json
import math
import os
import pathlib
import sys
import time
import zlib

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

DATA = pathlib.Path(os.path.expanduser("~/.cache/spirebot-data/naz1"))
CKPT = pathlib.Path(os.path.expanduser("~/spirebot/target/rl-checkpoints/NAZ1"))
LOSS_MIN, LOSS_MAX = -20, 119
NB_LOSS = LOSS_MAX - LOSS_MIN + 1
MAX_DECK, MAX_RELICS, MAX_POTIONS = 48, 40, 8
T_CLS, T_CTX, T_ENC, T_CARD, T_RELIC, T_POTION = range(6)
N_SCALARS = 9


# ---------------------------------------------------------------------------------------------
# Tokens


class Vocab:
    """Ids for each kind; 0 is padding, 1 `<unk>`."""

    KINDS = ("card", "relic", "potion", "encounter", "act_id")

    def __init__(self, tables=None):
        self.tables = tables or {k: {} for k in self.KINDS}

    @classmethod
    def build(cls, recs):
        counts = {k: collections.Counter() for k in cls.KINDS}
        for r in recs:
            counts["card"].update(c[0] for c in r["deck"])
            counts["relic"].update(r["relics"])
            counts["potion"].update(r["potions"])
            counts["encounter"][r["encounter"]] += 1
            counts["act_id"][r["act_id"]] += 1
        return cls({k: {name: i + 2 for i, name in enumerate(sorted(c))}
                    for k, c in counts.items()})

    def id(self, kind, name):
        return self.tables[kind].get(name, 1)

    def size(self, kind):
        return len(self.tables[kind]) + 2


def tokenize(vocab, recs):
    """Int/float arrays for a batch of records (see `FightNet.forward`)."""
    n = len(recs)
    cards = np.zeros((n, MAX_DECK, 3), dtype=np.int64)  # id, upgrade (0-2), count
    relics = np.zeros((n, MAX_RELICS), dtype=np.int64)
    potions = np.zeros((n, MAX_POTIONS), dtype=np.int64)
    enc = np.zeros(n, dtype=np.int64)
    act_id = np.zeros(n, dtype=np.int64)
    scal = np.zeros((n, N_SCALARS), dtype=np.float32)
    hp = np.zeros(n, dtype=np.int64)
    cv, rv, pv = vocab.tables["card"], vocab.tables["relic"], vocab.tables["potion"]
    for i, r in enumerate(recs):
        c = collections.Counter((cv.get(x[0], 1), min(int(x[1]), 2)) for x in r["deck"])
        for j, ((cid, up), k) in enumerate(sorted(c.items())[:MAX_DECK]):
            cards[i, j] = (cid, up, min(k, 15))
        rl = [rv.get(x, 1) for x in r["relics"]][:MAX_RELICS]
        relics[i, :len(rl)] = rl
        pl = [pv.get(x, 1) for x in r["potions"]][:MAX_POTIONS]
        potions[i, :len(pl)] = pl
        enc[i] = vocab.id("encounter", r["encounter"])
        act_id[i] = vocab.id("act_id", r["act_id"])
        mh = max(1, r["max_hp"])
        scal[i] = (r["hp"] / 100, mh / 100, r["hp"] / mh, r["floor"] / 50, r["act"] / 2,
                   min(r["gold"], 1000) / 500, len(r["deck"]) / 30, r["potion_slots"] / 4,
                   len(r["potions"]) / 4)
        hp[i] = r["hp"]
    return dict(cards=cards, relics=relics, potions=potions, enc=enc, act_id=act_id, scal=scal,
                hp=hp, act=np.array([r["act"] for r in recs], dtype=np.int64))


def to_dev(tok, dev, idx=None):
    """The batch on `dev`, its card, relic and potion columns cut to the longest row (padding
    is masked, so the output is the same; it saves most of the attention work)."""
    out = {}
    for k, v in tok.items():
        v = v if idx is None else v[idx]
        if k in ("cards", "relics", "potions"):
            used = (v[..., 0] if k == "cards" else v) != 0
            width = int(used.any(0).nonzero()[0].max()) + 1 if used.any() else 1
            v = v[:, :width]
        out[k] = torch.from_numpy(np.ascontiguousarray(v)).to(dev, non_blocking=True)
    return out


# ---------------------------------------------------------------------------------------------
# Net


LOSS_VALUES = torch.arange(LOSS_MIN, LOSS_MAX + 1, dtype=torch.float32)


class FightNet(nn.Module):
    def __init__(self, sizes, d=192, layers=4, heads=4, ff=4, dropout=0.0):
        super().__init__()
        self.sizes, self.d, self.layers, self.heads, self.ff = sizes, d, layers, heads, ff
        self.card = nn.Embedding(sizes["card"], d)
        self.up = nn.Embedding(3, d)
        self.count = nn.Embedding(16, d)
        self.relic = nn.Embedding(sizes["relic"], d)
        self.potion = nn.Embedding(sizes["potion"], d)
        self.enc = nn.Embedding(sizes["encounter"], d)
        self.act_id = nn.Embedding(sizes["act_id"], d)
        self.act = nn.Embedding(3, d)
        self.tok_type = nn.Embedding(6, d)
        self.cls = nn.Parameter(torch.zeros(d))
        self.ctx = nn.Sequential(nn.Linear(N_SCALARS, d), nn.GELU(), nn.Linear(d, d))
        layer = nn.TransformerEncoderLayer(d, heads, ff * d, dropout, activation="gelu",
                                           batch_first=True, norm_first=True)
        self.body = nn.TransformerEncoder(layer, layers, enable_nested_tensor=False)
        self.norm = nn.LayerNorm(d)
        self.head = nn.Sequential(nn.Linear(d, d), nn.GELU(), nn.Linear(d, NB_LOSS + 1))
        self.register_buffer("loss_values", LOSS_VALUES.clone(), persistent=False)

    def config(self):
        return dict(sizes=self.sizes, d=self.d, layers=self.layers, heads=self.heads, ff=self.ff)

    def forward(self, t):
        """Log-probabilities (n, NB_LOSS + 1): survival with HP lost `loss_values`, then death."""
        n = t["enc"].shape[0]
        ty = self.tok_type.weight
        cls = (self.cls + ty[T_CLS]).expand(n, 1, -1)
        ctx = (self.ctx(t["scal"]) + self.act(t["act"]) + self.act_id(t["act_id"]) + ty[T_CTX])
        enc = self.enc(t["enc"]) + ty[T_ENC]
        c = t["cards"]
        cards = self.card(c[..., 0]) + self.up(c[..., 1]) + self.count(c[..., 2]) + ty[T_CARD]
        relics = self.relic(t["relics"]) + ty[T_RELIC]
        potions = self.potion(t["potions"]) + ty[T_POTION]
        x = torch.cat([cls, ctx[:, None], enc[:, None], cards, relics, potions], 1)
        pad = torch.cat([torch.zeros(n, 3, dtype=torch.bool, device=x.device), c[..., 0] == 0,
                         t["relics"] == 0, t["potions"] == 0], 1)
        x = self.body(x, src_key_padding_mask=pad)
        logits = self.head(self.norm(x[:, 0])).float()
        # Surviving with HP lost >= the entry HP is impossible.
        bad = self.loss_values[None, :] >= t["hp"][:, None].float()
        logits[:, :NB_LOSS] = logits[:, :NB_LOSS].masked_fill(bad, float("-inf"))
        return F.log_softmax(logits, -1)

    def summary(self, logp, hp):
        """(p_death, expected HP loss, probabilities)."""
        p = logp.exp()
        mean = (p[:, :NB_LOSS] * self.loss_values).sum(-1) + p[:, NB_LOSS] * hp.float()
        return p[:, NB_LOSS], mean, p


class FightModel:
    """A trained model and its vocab, for prediction."""

    def __init__(self, net, vocab, device):
        self.net, self.vocab, self.device = net, vocab, device
        self.loss_values = LOSS_VALUES.numpy()

    @classmethod
    def load(cls, path, device="cuda"):
        path = pathlib.Path(path)
        state = torch.load(path / "model.pt", map_location="cpu", weights_only=False)
        net = FightNet(**state["config"])
        net.load_state_dict(state["net"])
        return cls(net.to(device).eval(), Vocab(state["vocab"]), torch.device(device))

    @torch.no_grad()
    def predict_tokens(self, tok, chunk=4096):
        out = collections.defaultdict(list)
        n = len(tok["enc"])
        for k in range(0, n, chunk):
            t = to_dev(tok, self.device, slice(k, k + chunk))
            with torch.autocast(self.device.type, dtype=torch.bfloat16,
                                enabled=self.device.type == "cuda"):
                logp = self.net(t)
            pd, mean, p = self.net.summary(logp, t["hp"])
            out["p_death"].append(pd.cpu().numpy())
            out["mean_loss"].append(mean.cpu().numpy())
            out["hist"].append(p.cpu().numpy())
        return {k: np.concatenate(v) for k, v in out.items()}

    def predict(self, recs, chunk=4096):
        return self.predict_tokens(tokenize(self.vocab, recs), chunk)


# ---------------------------------------------------------------------------------------------
# Data


def load_set(name):
    """(records, labels dict) of a labelled set: `<name>.jsonl` + `<name>.npz` in DATA."""
    recs = [json.loads(line) for line in open(DATA / f"{name}.jsonl") if line.strip()]
    z = np.load(DATA / f"{name}.npz")
    assert len(z["hp_lost"]) == len(recs), name
    assert list(z["ids"][:5]) == [r["id"] for r in recs[:5]], name
    bad = z["bad"] if "bad" in z.files else np.zeros(z["died"].shape, dtype=bool)
    return recs, dict(hp_lost=z["hp_lost"].astype(np.int64), died=z["died"].astype(bool), ok=~bad)


def loss_of(recs, lab):
    """Per rollout HP lost, a death losing the entry HP."""
    return np.where(lab["died"], np.array([r["hp"] for r in recs])[:, None], lab["hp_lost"])


def rmean(x, ok):
    """The mean over the valid rollouts of each start."""
    return (x * ok).sum(1) / np.maximum(ok.sum(1), 1)


def targets(recs, lab):
    """Empirical histograms (n, NB_LOSS + 1) and mean losses (death = entry HP), over the valid
    rollouts."""
    h, d, ok = lab["hp_lost"], lab["died"], lab["ok"]
    n, reps = h.shape
    bins = np.where(d, NB_LOSS, np.clip(h, LOSS_MIN, LOSS_MAX) - LOSS_MIN)
    hist = np.zeros((n, NB_LOSS + 1), dtype=np.float32)
    np.add.at(hist, (np.repeat(np.arange(n), reps), bins.ravel()), ok.ravel().astype(np.float32))
    hist /= np.maximum(ok.sum(1, keepdims=True), 1)
    return hist, rmean(loss_of(recs, lab), ok).astype(np.float32)


TRAIN_SETS = ("human_gym", "own_train", "pert_train")
VALID_FRAC = 0.04


def group_of(r):
    if "base" in r:
        return r["base"].split("-")[0]
    return r["id"].split("-")[0]


def cmd_train(args):
    torch.manual_seed(args.seed)
    dev = torch.device("cuda")
    torch.cuda.set_per_process_memory_fraction(args.gpu_frac)
    set_weight = {k: float(v) for k, v in (kv.split("=") for kv in args.set_weights.split(",") if kv)}
    recs, hists, means, src_w = [], [], [], []
    for name in args.sets.split(","):
        if not (DATA / f"{name}.npz").exists():
            print(f"skip {name}: not labelled")
            continue
        r, lab = load_set(name)
        h, m = targets(r, lab)
        if (~lab["ok"]).any():
            print(f"{name}: {(~lab['ok']).sum()} bad rollouts excluded")
        recs += r
        hists.append(h)
        means.append(m)
        src_w.append(np.full(len(r), set_weight.get(name, 1.0), dtype=np.float32))
        print(f"{name}: {len(r)} starts x {lab['hp_lost'].shape[1]}")
    hist, mean = np.concatenate(hists), np.concatenate(means)
    room = np.array([room_of(r) for r in recs])
    weight = np.where(room == "boss", args.boss_weight,
                      np.where(room == "elite", args.elite_weight, 1.0)).astype(np.float32)
    weight *= np.concatenate(src_w)
    groups = np.array([zlib.crc32(group_of(r).encode()) % 10_000 for r in recs])
    valid = groups < VALID_FRAC * 10_000
    vocab = Vocab.build([r for r, v in zip(recs, valid) if not v])
    tok = tokenize(vocab, recs)
    tr, va = np.nonzero(~valid)[0], np.nonzero(valid)[0]
    print(f"train {len(tr)}, valid {len(va)}; vocab "
          f"{ {k: vocab.size(k) for k in Vocab.KINDS} }")
    sizes = {k: vocab.size(k) for k in Vocab.KINDS}
    net = FightNet(sizes, d=args.d, layers=args.layers, heads=args.heads,
                   dropout=args.dropout).to(dev)
    opt = torch.optim.AdamW(net.parameters(), lr=args.lr, weight_decay=args.wd)
    steps = args.epochs * math.ceil(len(tr) / args.batch)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, args.lr, total_steps=steps, pct_start=0.05)
    gtok = to_dev(tok, dev)
    ghist = torch.from_numpy(hist).to(dev)
    gmean = torch.from_numpy(mean).to(dev)
    gweight = torch.from_numpy(weight).to(dev)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    best, log = float("inf"), []
    t0 = time.time()

    def batch_loss(idx, train):
        t = {k: v[idx] for k, v in gtok.items()}
        with torch.autocast("cuda", dtype=torch.bfloat16):
            logp = net(t)
        target = ghist[idx]
        ce = -(target * logp.clamp_min(-1e4)).sum(-1)
        _, m, _ = net.summary(logp, t["hp"])
        reg = F.smooth_l1_loss(m, gmean[idx], beta=2.0, reduction="none")
        return ce, reg, m

    for ep in range(args.epochs):
        net.train()
        perm = torch.from_numpy(np.random.default_rng(args.seed + ep).permutation(tr)).to(dev)
        tot = 0.0
        for k in range(0, len(perm), args.batch):
            idx = perm[k:k + args.batch]
            ce, reg, _ = batch_loss(idx, True)
            w = gweight[idx]
            loss = ((ce + args.reg * reg) * w).sum() / w.sum()
            opt.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(net.parameters(), 1.0)
            opt.step()
            sched.step()
            tot += loss.item() * len(idx)
        net.eval()
        with torch.no_grad():
            vce, vmae, vn = 0.0, 0.0, 0
            for k in range(0, len(va), 4096):
                idx = torch.from_numpy(va[k:k + 4096]).to(dev)
                ce, _, m = batch_loss(idx, False)
                vce += ce.sum().item()
                vmae += (m - gmean[idx]).abs().sum().item()
                vn += len(idx)
        row = dict(epoch=ep, train=tot / len(tr), valid_ce=vce / vn, valid_mae=vmae / vn,
                   seconds=round(time.time() - t0))
        log.append(row)
        print(json.dumps(row), flush=True)
        if row["valid_ce"] < best:
            best = row["valid_ce"]
            torch.save(dict(config=net.config(), net=net.state_dict(), vocab=vocab.tables,
                            args=vars(args), epoch=ep), out / "model.pt")
    (out / "train_log.json").write_text(json.dumps(dict(args=vars(args), log=log), indent=1))
    # The vocab as JSON, for a non-Python tokenizer (NAZ2 from Rust).
    (out / "vocab.json").write_text(json.dumps(dict(
        tables=vocab.tables, max_deck=MAX_DECK, max_relics=MAX_RELICS, max_potions=MAX_POTIONS,
        loss_min=LOSS_MIN, loss_max=LOSS_MAX), indent=0))
    return 0


# ---------------------------------------------------------------------------------------------
# Eval


def ece(p, y, bins=15):
    edges = np.linspace(0, 1, bins + 1)
    k = np.clip(np.digitize(p, edges) - 1, 0, bins - 1)
    e = 0.0
    for b in range(bins):
        s = k == b
        if s.any():
            e += s.mean() * abs(p[s].mean() - y[s].mean())
    return e


def auc(score, label):
    """Rank AUC (ties averaged)."""
    order = np.argsort(score, kind="mergesort")
    s = score[order]
    r = np.empty(len(s), dtype=np.float64)
    starts = np.r_[0, np.nonzero(np.diff(s))[0] + 1]
    ends = np.r_[starts[1:], len(s)]
    for a, b in zip(starts, ends):
        r[order[a:b]] = (a + b + 1) / 2
    pos = label.astype(bool)
    n1, n0 = pos.sum(), (~pos).sum()
    return (r[pos].sum() - n1 * (n1 + 1) / 2) / (n1 * n0)


def room_of(r):
    e = r["encounter"]
    return "boss" if e.endswith("_BOSS") else "elite" if e.endswith("_ELITE") else "hallway"


def metrics(recs, lab, pred):
    d, ok = lab["died"], lab["ok"]
    death_rate = rmean(d, ok)
    loss = loss_of(recs, lab)
    mean = rmean(loss, ok)
    err = np.abs(pred["mean_loss"] - mean)
    reps = d.shape[1]
    okr = ok.ravel()
    out = dict(n=len(recs), reps=int(reps), bad_rollouts=int((~ok).sum()),
               ece=ece(pred["p_death"], death_rate),
               auc=auc(np.repeat(pred["p_death"], reps)[okr], d.ravel()[okr]),
               mae=float(err.mean()), true_death=float(death_rate.mean()),
               pred_death=float(pred["p_death"].mean()), true_loss=float(mean.mean()),
               pred_loss=float(pred["mean_loss"].mean()))
    rooms = np.array([room_of(r) for r in recs])
    acts = np.array([r["act"] for r in recs])
    for a in range(3):
        out[f"mae_act{a + 1}"] = float(err[acts == a].mean())
    for room in ("hallway", "elite", "boss"):
        s = rooms == room
        out[f"mae_{room}"] = float(err[s].mean())
        out[f"ece_{room}"] = ece(pred["p_death"][s], death_rate[s])
        out[f"n_{room}"] = int(s.sum())
    # The label noise floor: the expected |mean of reps - true mean| from the rollouts' spread.
    var = rmean((loss - mean[:, None]) ** 2, ok) * reps / max(reps - 1, 1)
    out["noise_mae"] = float((np.sqrt(var / reps) * math.sqrt(2 / math.pi)).mean())
    return out


def cf_metrics(fm, base_name, kinds, thresh=2.0):
    """Sign agreement of predicted vs rollout change in expected HP loss, on aligned pairs."""
    brecs, blab = load_set(base_name)
    bp = fm.predict(brecs)
    bmean = rmean(loss_of(brecs, blab), blab["ok"])
    by_id = {r["id"]: i for i, r in enumerate(brecs)}
    out, allagree = {}, []
    for kind in kinds:
        name = f"{base_name}_cf_{kind}"
        if not (DATA / f"{name}.npz").exists():
            continue
        vrecs, vlab = load_set(name)
        vp = fm.predict(vrecs)
        vmean = rmean(loss_of(vrecs, vlab), vlab["ok"])
        bi = np.array([by_id[r["base"]] for r in vrecs])
        true_d = vmean - bmean[bi]
        pred_d = vp["mean_loss"] - bp["mean_loss"][bi]
        s = np.abs(true_d) > thresh
        agree = np.sign(true_d[s]) == np.sign(pred_d[s])
        allagree.append(agree)
        out[kind] = dict(pairs=len(vrecs), big=int(s.sum()), agree=float(agree.mean()),
                         corr=float(np.corrcoef(true_d, pred_d)[0, 1]))
    if allagree:
        a = np.concatenate(allagree)
        out["all"] = dict(big=len(a), agree=float(a.mean()))
    return out


TEST_SETS = ("human_panel", "own_test")


def cmd_eval(args):
    fm = FightModel.load(args.dir)
    res = {}
    allr, alll, allp = [], dict(hp_lost=[], died=[], ok=[]), collections.defaultdict(list)
    for name in TEST_SETS:
        if not (DATA / f"{name}.npz").exists():
            continue
        recs, lab = load_set(name)
        pred = fm.predict(recs)
        res[name] = metrics(recs, lab, pred)
        allr += recs
        for k in alll:
            alll[k].append(lab[k])
        for k, v in pred.items():
            allp[k].append(v)
    if len(alll["died"]) > 1 and len({x.shape[1] for x in alll["died"]}) == 1:
        res["all"] = metrics(allr, {k: np.concatenate(v) for k, v in alll.items()},
                             {k: np.concatenate(v) for k, v in allp.items()})
    res["cf"] = cf_metrics(fm, args.cf_base, ("add", "upgrade", "remove"), args.thresh) \
        if (DATA / f"{args.cf_base}.npz").exists() else {}
    text = json.dumps(res, indent=1)
    print(text)
    (pathlib.Path(args.dir) / "eval.json").write_text(text)
    return 0


def cmd_speed(args):
    fm = FightModel.load(args.dir)
    recs, _ = load_set("human_panel")
    recs = (recs * (args.n // len(recs) + 1))[:args.n]
    t0 = time.time()
    tok = tokenize(fm.vocab, recs)
    t_tok = time.time() - t0
    for _ in range(2):
        fm.predict_tokens(tok, args.chunk)
    torch.cuda.synchronize()
    t0 = time.time()
    for _ in range(args.iters):
        fm.predict_tokens(tok, args.chunk)
    torch.cuda.synchronize()
    dt = (time.time() - t0) / args.iters
    res = dict(n=args.n, chunk=args.chunk, evals_per_s_gpu=args.n / dt,
               tokenize_per_s=args.n / t_tok, end_to_end_per_s=args.n / (dt + t_tok))
    print(json.dumps(res))
    (pathlib.Path(args.dir) / "speed.json").write_text(json.dumps(res, indent=1))
    return 0


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    t = sub.add_parser("train")
    t.add_argument("--out", required=True)
    t.add_argument("--sets", default=",".join(TRAIN_SETS))
    t.add_argument("--epochs", type=int, default=30)
    t.add_argument("--batch", type=int, default=1024)
    t.add_argument("--lr", type=float, default=5e-4)
    t.add_argument("--wd", type=float, default=0.05)
    t.add_argument("--d", type=int, default=192)
    t.add_argument("--layers", type=int, default=4)
    t.add_argument("--heads", type=int, default=4)
    t.add_argument("--dropout", type=float, default=0.0)
    t.add_argument("--reg", type=float, default=0.05)
    t.add_argument("--seed", type=int, default=0)
    t.add_argument("--boss-weight", type=float, default=1.0)
    t.add_argument("--elite-weight", type=float, default=1.0)
    t.add_argument("--gpu-frac", type=float, default=0.5)
    t.add_argument("--set-weights", default="",
                   help="per-set loss weights, e.g. human_gym=3,pert_human=2")
    e = sub.add_parser("eval")
    e.add_argument("dir")
    e.add_argument("--cf-base", default="cf_base")
    e.add_argument("--thresh", type=float, default=2.0)
    s = sub.add_parser("speed")
    s.add_argument("dir")
    s.add_argument("--n", type=int, default=65536)
    s.add_argument("--chunk", type=int, default=16384)
    s.add_argument("--iters", type=int, default=5)
    args = p.parse_args()
    return {"train": cmd_train, "eval": cmd_eval, "speed": cmd_speed}[args.cmd](args)


if __name__ == "__main__":
    raise SystemExit(main())
