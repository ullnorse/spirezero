"""RLN41: a deckbuilding prior from strong human Necrobinder A10 players.

    python python/rln41_prior.py train [--expert] [--wins-only] [--epochs 12]
    python python/rln41_prior.py eval --heldout      # per-kind top-1 accuracy vs the baseline
    python python/rln41_prior.py report              # rates, rankings, Nave checks (-> DATA/report.json)
    python python/rln41_prior.py adapter CKPT --weights ema0.999 --runs 40   # the bot vs the prior

Data and weights live in `~/.cache/spirebot-data/rln41/` (`rln41_dataset.DATA`): never committed.

**Model** (`Prior`): a card is an embedding of its vocab id plus its upgrade (and, with `expert`, a
linear map of Nave's tier and upgrade priority for that card, `nave_table`). The deck is a set: a
2-layer MLP per card, pooled as mean and sum / 20; relics are a mean embedding; with 9 scalars (HP
fraction, max HP / 100, gold / 300, act one-hot, floor / 50, deck size / 30, relics / 15) and the
decision kind they make a context `c`. An option is its token type's embedding plus its card, relic
or event-key embedding and the count of that card already in the deck; its score is an MLP of
`[c, o, P c * o]`. Cross-entropy over a record's options, AdamW, 2 CPU threads.

**Split:** `rln41_dataset.HELDOUT` players (6 of 37) are never trained on. The baseline picks the option
with the highest training pick rate (picks / offers of that (type, id, upgrade)), which for a fixed
option set (rest, shop removal) is the majority class.

**Adapter** (`obs_record`): the same record from one row of the env's run obs (`run_obs.rs` layout:
header, deck, relics, rewards, rest, select, shop, ancient, event sections) and the action the bot
took (`bot_choice`); no new engine accessor, so the FFI build is unchanged.
"""

import argparse
import collections
import gzip
import json
import math
import os
import pathlib
import pickle
import random
import sys

import numpy as np
import torch
from torch import nn

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import rln41_dataset as D  # noqa: E402

DATA = D.DATA
NAVE = pathlib.Path(os.path.expanduser("~/spirebot/target/nave"))
NAVE_TIER = NAVE / "necro-tier.json"
NAVE_CHECKS = DATA / "nave_checks.json"
KIND_ID = {k: i for i, k in enumerate(D.KINDS)}
CARD_ROWS, RELIC_ROWS, EVENT_BUCKETS = 1024, 512, 2048
N_SCALARS = 9
# Nave's most-upgraded cards (summary_B.md), in his order.
NAVE_TOP_UPGRADES = ["SQUEEZE", "BORROWED_TIME", "ERADICATE", "BODYGUARD", "DELAY", "DEFY", "SHROUD",
                     "DANSE_MACABRE", "DREDGE", "LETHALITY"]


def model_path(expert, wins_only):
    return DATA / f"prior{'-expert' if expert else ''}{'-wins' if wins_only else ''}.pt"


# ---------------------------------------------------------------------------------------------
# Nave expert features


def tier_value(t):
    """'S' 6 ... 'F' 1, '+' / '-' +-1/3; None for no tier."""
    if not t:
        return None
    base = {"S": 6, "A": 5, "B": 4, "C": 3, "D": 2, "F": 1}.get(t.strip()[0].upper())
    if base is None:
        return None
    return base + (t.count("+") - t.count("-")) / 3


UPGRADE_PRIORITY = {"high": 3, "medium": 2, "med": 2, "low": 1}


def nave_cards(path=NAVE_TIER):
    """{CARD_ID: (tier or None, upgrade priority or None)} from Nave's tier list (v0.111, else v0.107,
    else older)."""
    if not path.exists():
        return {}
    out = {}
    for c in json.loads(path.read_text()):
        tier = next((tier_value(c.get(k)) for k in ("tier_v0111", "tier_v0107", "tier_old")
                     if tier_value(c.get(k)) is not None), None)
        up = UPGRADE_PRIORITY.get(str(c.get("upgrade_priority") or "").lower())
        out[c["card_id"].upper()] = (tier, up)
    return out


def nave_table(cv=None):
    """[CARD_ROWS, 4]: tier / 6, tier unknown, upgrade priority / 3, priority unknown."""
    cv = cv or D.cards()
    t = np.zeros((CARD_ROWS, 4), dtype=np.float32)
    t[:, 1] = t[:, 3] = 1
    for card, (tier, up) in nave_cards().items():
        i = cv(card)
        if i <= 1:
            continue
        if tier is not None:
            t[i, 0], t[i, 1] = tier / 6, 0
        if up is not None:
            t[i, 2], t[i, 3] = up / 3, 0
    return torch.from_numpy(t)


# ---------------------------------------------------------------------------------------------
# Tensors


def collate(recs):
    b = len(recs)
    dmax = max(1, max(len(r["deck"]) for r in recs))
    rmax = max(1, max(len(r["relics"]) for r in recs))
    omax = max(len(r["options"]) for r in recs)
    deck = np.zeros((b, dmax, 2), dtype=np.int64)
    rel = np.zeros((b, rmax), dtype=np.int64)
    scal = np.zeros((b, N_SCALARS), dtype=np.float32)
    kind = np.zeros(b, dtype=np.int64)
    opt = np.zeros((b, omax, 3), dtype=np.int64)
    cnt = np.zeros((b, omax), dtype=np.float32)
    omask = np.zeros((b, omax), dtype=bool)
    choice = np.zeros(b, dtype=np.int64)
    for i, r in enumerate(recs):
        if r["deck"]:
            deck[i, :len(r["deck"])] = r["deck"]
        if r["relics"]:
            rel[i, :len(r["relics"])] = r["relics"]
        mhp = max(1, r["max_hp"])
        act = min(max(r["act"], 0), 2)
        scal[i] = [r["hp"] / mhp, mhp / 100, r["gold"] / 300, act == 0, act == 1, act == 2,
                   r["floor"] / 50, len(r["deck"]) / 30, len(r["relics"]) / 15]
        kind[i] = KIND_ID[r["kind"]]
        o = r["options"]
        opt[i, :len(o)] = o
        omask[i, :len(o)] = True
        c = collections.Counter(cid for cid, _ in r["deck"])
        cnt[i, :len(o)] = [c[oid] if t == D.T_CARD else 0 for t, oid, _ in o]
        choice[i] = r.get("choice", 0)
    t = torch.from_numpy
    return dict(deck=t(deck), rel=t(rel), scal=t(scal), kind=t(kind), opt=t(opt), cnt=t(cnt),
                omask=t(omask), choice=t(choice))


class Prior(nn.Module):
    def __init__(self, expert=False, d=64):
        super().__init__()
        self.expert = expert
        self.card = nn.Embedding(CARD_ROWS, d)
        self.up = nn.Embedding(4, d)
        self.relic = nn.Embedding(RELIC_ROWS, d)
        self.event = nn.Embedding(EVENT_BUCKETS, d)
        self.typ = nn.Embedding(16, d)
        self.kind = nn.Embedding(16, 16)
        self.register_buffer("etab", nave_table() if expert else torch.zeros(CARD_ROWS, 4))
        self.exp = nn.Linear(4, d) if expert else None
        self.phi = nn.Sequential(nn.Linear(d, 2 * d), nn.ReLU(), nn.Linear(2 * d, d))
        self.ctx = nn.Sequential(nn.Linear(2 * d + d + N_SCALARS + 16, 2 * d), nn.ReLU(),
                                 nn.Linear(2 * d, 2 * d), nn.ReLU())
        self.optf = nn.Linear(d + 1, d)
        self.proj = nn.Linear(2 * d, d)
        self.head = nn.Sequential(nn.Linear(4 * d, 2 * d), nn.ReLU(), nn.Linear(2 * d, 1))

    def card_rep(self, ids, ups):
        ids = ids.clamp(0, CARD_ROWS - 1)
        r = self.card(ids) + self.up(ups.clamp(0, 3))
        if self.exp is not None:
            r = r + self.exp(self.etab[ids])
        return r

    def forward(self, x):
        dm = (x["deck"][..., 0] > 0).float().unsqueeze(-1)
        h = self.phi(self.card_rep(x["deck"][..., 0], x["deck"][..., 1])) * dm
        n = dm.sum(1).clamp(min=1)
        deck = torch.cat([h.sum(1) / n, h.sum(1) / 20], -1)
        rm = (x["rel"] > 0).float().unsqueeze(-1)
        rel = (self.relic(x["rel"].clamp(0, RELIC_ROWS - 1)) * rm).sum(1) / rm.sum(1).clamp(min=1)
        c = self.ctx(torch.cat([deck, rel, x["scal"], self.kind(x["kind"])], -1))
        t, i, u = x["opt"][..., 0], x["opt"][..., 1], x["opt"][..., 2]
        rep = self.typ(t)
        rep = rep + self.card_rep(i, u) * (t == D.T_CARD).unsqueeze(-1)
        rep = rep + self.relic(i.clamp(0, RELIC_ROWS - 1)) * (t == D.T_RELIC).unsqueeze(-1)
        rep = rep + self.event(i % EVENT_BUCKETS) * (t == D.T_EVENT).unsqueeze(-1)
        o = self.optf(torch.cat([rep, x["cnt"].unsqueeze(-1) / 4], -1))
        ce = c.unsqueeze(1).expand(-1, o.shape[1], -1)
        pc = self.proj(c).unsqueeze(1)
        logits = self.head(torch.cat([ce, o, pc * o], -1)).squeeze(-1)
        return logits.masked_fill(~x["omask"], -1e9)


def load_records():
    with gzip.open(D.DECISIONS, "rb") as f:
        return pickle.load(f)


def train(args):
    torch.set_num_threads(2)
    torch.manual_seed(41)
    data = load_records()
    recs = [r for r in data["records"] if not r["heldout"] and (r["win"] or not args.wins_only)]
    rng = random.Random(41)
    net = Prior(expert=args.expert)
    opt = torch.optim.AdamW(net.parameters(), lr=args.lr, weight_decay=1e-4)
    steps = args.epochs * math.ceil(len(recs) / args.batch)
    sched = torch.optim.lr_scheduler.OneCycleLR(opt, max_lr=args.lr, total_steps=steps, pct_start=0.1)
    for ep in range(args.epochs):
        rng.shuffle(recs)
        tot, n = 0.0, 0
        net.train()
        for k in range(0, len(recs), args.batch):
            x = collate(recs[k:k + args.batch])
            loss = nn.functional.cross_entropy(net(x), x["choice"])
            opt.zero_grad()
            loss.backward()
            opt.step()
            sched.step()
            tot += loss.item() * len(x["choice"])
            n += len(x["choice"])
        acc = accuracy(net, [r for r in data["records"] if r["heldout"]])
        print(f"epoch {ep + 1}: train loss {tot / n:.4f}, held-out top-1 {acc['all']:.4f}", flush=True)
    path = model_path(args.expert, args.wins_only)
    torch.save(dict(net=net.state_dict(), expert=args.expert, wins_only=args.wins_only,
                    epochs=args.epochs, n_train=len(recs)), path)
    print(f"-> {path}")


def load_model(path):
    s = torch.load(path, map_location="cpu", weights_only=False)
    net = Prior(expert=s["expert"])
    net.load_state_dict(s["net"])
    return net.eval()


@torch.no_grad()
def scores(net, recs, batch=1024):
    """Each record's option probabilities (lists)."""
    out = []
    for k in range(0, len(recs), batch):
        part = recs[k:k + batch]
        p = torch.softmax(net(collate(part)), -1).numpy()
        out.extend(p[i, :len(r["options"])] for i, r in enumerate(part))
    return out


def accuracy(net, recs):
    net.eval()
    p = scores(net, recs)
    by = collections.defaultdict(list)
    for r, q in zip(recs, p):
        by[r["kind"]].append(int(np.argmax(q)) == r["choice"])
    out = {k: float(np.mean(v)) for k, v in by.items()}
    out["all"] = float(np.mean([x for v in by.values() for x in v]))
    return out


class Baseline:
    """Highest training pick rate per (kind, option token)."""

    def __init__(self, recs):
        self.off, self.pick = collections.Counter(), collections.Counter()
        for r in recs:
            for j, o in enumerate(r["options"]):
                key = (r["kind"], *o)
                self.off[key] += 1
                self.pick[key] += j == r["choice"]

    def predict(self, r):
        rate = [(self.pick[(r["kind"], *o)] + 0.5) / (self.off[(r["kind"], *o)] + 1) for o in r["options"]]
        return int(np.argmax(rate))


# ---------------------------------------------------------------------------------------------
# Eval and report


def variants():
    return [(e, w) for e in (False, True) for w in (False, True) if model_path(e, w).exists()]


def evaluate(args):
    data = load_records()
    recs = data["records"]
    held = [r for r in recs if r["heldout"]]
    base = Baseline([r for r in recs if not r["heldout"]])
    kinds = [k for k in D.KINDS if any(r["kind"] == k for r in held)]
    cols = [("baseline", None)] + [(model_path(e, w).stem, (e, w)) for e, w in variants()]
    accs = {}
    for name, v in cols:
        if v is None:
            by = collections.defaultdict(list)
            for r in held:
                by[r["kind"]].append(base.predict(r) == r["choice"])
            a = {k: float(np.mean(x)) for k, x in by.items()}
            a["all"] = float(np.mean([x for xs in by.values() for x in xs]))
        else:
            a = accuracy(load_model(model_path(*v)), held)
        accs[name] = a
    wins = [r for r in held if r["win"]]
    print(f"held-out: {len(held)} decisions, {len(set(r['player'] for r in held))} players "
          f"({len(wins)} in wins)")
    print(f"{'kind':14}{'n':>7}" + "".join(f"{n:>20}" for n, _ in cols))
    for k in kinds + ["all"]:
        n = sum(r["kind"] == k for r in held) if k != "all" else len(held)
        print(f"{k:14}{n:7}" + "".join(f"{accs[c][k]:20.3f}" for c, _ in cols))
    macro = {c: np.mean([accs[c][k] for k in kinds]) for c, _ in cols}
    print(f"{'macro':14}{'':7}" + "".join(f"{macro[c]:20.3f}" for c, _ in cols))
    if wins:
        print("held-out wins only, all kinds:", "  ".join(
            f"{model_path(*v).stem} {accuracy(load_model(model_path(*v)), wins)['all']:.3f}"
            for _, v in cols if v))
    (DATA / "eval.json").write_text(json.dumps(accs, indent=1))


def hp_band(r):
    f = r["hp"] / max(1, r["max_hp"])
    for lo, name in ((0.9, ">=90"), (0.7, "70-89"), (0.5, "50-69"), (0.3, "30-49")):
        if f >= lo:
            return name
    return "<30"


BANDS = ["<30", "30-49", "50-69", "70-89", ">=90"]


def rates(net, recs):
    """Smith by HP band, skip, removal use and removal targets: human vs prior (argmax, mean P)."""
    p = scores(net, recs)
    out = {}
    rest = [(r, q) for r, q in zip(recs, p) if r["kind"] == "rest"]
    sm = {}
    for b in BANDS + ["all"]:
        rr = [(r, q) for r, q in rest if b == "all" or hp_band(r) == b]
        if not rr:
            continue
        def smith_idx(r):
            return [t for t, _, _ in r["options"]].index(D.T_SMITH) if any(
                t == D.T_SMITH for t, _, _ in r["options"]) else None
        hs = [r["choice"] == smith_idx(r) for r, _ in rr]
        ps = [int(np.argmax(q)) == smith_idx(r) for r, q in rr]
        ms = [q[smith_idx(r)] if smith_idx(r) is not None else 0 for r, q in rr]
        sm[b] = dict(n=len(rr), human=float(np.mean(hs)), prior=float(np.mean(ps)), p=float(np.mean(ms)))
    out["smith"] = sm
    cr = [(r, q) for r, q in zip(recs, p) if r["kind"] == "card_reward"]
    if cr:
        sk = [len(r["options"]) - 1 for r, _ in cr]
        out["skip"] = dict(n=len(cr), human=float(np.mean([r["choice"] == s for (r, _), s in zip(cr, sk)])),
                           prior=float(np.mean([int(np.argmax(q)) == s for (_, q), s in zip(cr, sk)])),
                           p=float(np.mean([q[s] for (_, q), s in zip(cr, sk)])))
    su = [(r, q) for r, q in zip(recs, p) if r["kind"] == "shop_removal"]
    if su:
        out["removal_use"] = dict(n=len(su), human=float(np.mean([r["choice"] == 0 for r, _ in su])),
                                  prior=float(np.mean([int(np.argmax(q)) == 0 for _, q in su])),
                                  p=float(np.mean([q[0] for _, q in su])))
    cv = D.cards()
    rm = [(r, q) for r, q in zip(recs, p) if r["kind"] == "shop_remove"]
    if rm:
        h = collections.Counter(cv.name(r["options"][r["choice"]][1]) for r, _ in rm)
        pr = collections.Counter(cv.name(r["options"][int(np.argmax(q))][1]) for r, q in rm)
        out["remove_targets"] = dict(n=len(rm), human=h.most_common(6), prior=pr.most_common(6))
    return out


def spearman(a, b):
    def rank(x):
        x = np.asarray(x, dtype=float)
        o = np.argsort(x, kind="stable")
        r = np.empty(len(x))
        r[o] = np.arange(len(x))
        for v in np.unique(x):  # average ties
            m = x == v
            r[m] = r[m].mean()
        return r
    ra, rb = rank(a), rank(b)
    return float(np.corrcoef(ra, rb)[0, 1]) if len(a) > 2 else float("nan")


@torch.no_grad()
def card_ranking(net, recs, n_ctx=400, min_offers=40):
    """{card: mean logit(card) - logit(skip)} over `n_ctx` real card-reward states, each card offered
    alone with a skip."""
    cr = [r for r in recs if r["kind"] == "card_reward"]
    offers = collections.Counter(o[1] for r in cr for o in r["options"] if o[0] == D.T_CARD)
    pool = sorted(c for c, n in offers.items() if n >= min_offers and c > 1)
    ctx = random.Random(41).sample(cr, min(n_ctx, len(cr)))
    out = {}
    for c in pool:
        rs = [dict(r, options=[(D.T_CARD, c, 0), (D.T_SKIP, 0, 0)], choice=0) for r in ctx]
        lg = net(collate(rs))
        out[c] = float((lg[:, 0] - lg[:, 1]).mean())
    return out


@torch.no_grad()
def smith_ranking(net, recs, min_ctx=50):
    """{card: (prior lift, human lift, n)} over Smith-target states offering it; lift is P(card) x
    options (1 = chance), the human's is chosen x options."""
    st = [r for r in recs if r["kind"] == "smith_target"]
    prior, human = collections.defaultdict(list), collections.defaultdict(list)
    for r, q in zip(st, scores(net, st)):
        n = len(r["options"])
        for j, (_, c, _) in enumerate(r["options"]):
            prior[c].append(float(q[j]) * n)
            human[c].append(float(j == r["choice"]) * n)
    return {c: (float(np.mean(v)), float(np.mean(human[c])), len(v)) for c, v in prior.items()
            if len(v) >= min_ctx and c > 1}


def nave_compare(cards_rank, smith_rank):
    cv = D.cards()
    nave = nave_cards()
    tiers = [(c, nave[cv.name(c)][0]) for c in cards_rank if nave.get(cv.name(c), (None,))[0] is not None]
    rho_tier = spearman([cards_rank[c] for c, _ in tiers], [t for _, t in tiers])
    ups = [(c, nave[cv.name(c)][1]) for c in smith_rank if nave.get(cv.name(c), (None, None))[1] is not None]
    rho_up = spearman([smith_rank[c][0] for c, _ in ups], [u for _, u in ups])
    rho_up_human = spearman([smith_rank[c][1] for c, _ in ups], [u for _, u in ups])
    rho_human = spearman([v[0] for v in smith_rank.values()], [v[1] for v in smith_rank.values()])
    top_smith = [cv.name(c) for c in sorted(smith_rank, key=lambda c: smith_rank[c][0], reverse=True)]
    top_human = [cv.name(c) for c in sorted(smith_rank, key=lambda c: smith_rank[c][1], reverse=True)]
    overlap = len(set(top_smith[:10]) & set(NAVE_TOP_UPGRADES))
    return dict(rho_tier=rho_tier, n_tier=len(tiers), rho_upgrade=rho_up, n_upgrade=len(ups),
                rho_upgrade_human=rho_up_human, rho_smith_prior_human=rho_human, n_smith=len(smith_rank),
                smith_top10_overlap=overlap, smith_top10=top_smith[:10], human_smith_top10=top_human[:10],
                human_top10_overlap=len(set(top_human[:10]) & set(NAVE_TOP_UPGRADES)),
                nave_top10_ranks={n: (top_smith.index(n) + 1 if n in top_smith else None)
                                  for n in NAVE_TOP_UPGRADES})


# ---- Nave's concrete decisions (validation only) ----

def nave_state(d):
    deck = [(c, 0) for c in D.START_DECK] + [(c, 0) for c in d.get("known_deck", [])]
    hp_frac = (d["hp"] / d["max_hp"] if d.get("hp") and d.get("max_hp") else
               0.3 if d.get("hp_low") else 0.85 if d.get("hp_low") is False else 0.7)
    mhp = d.get("max_hp") or 70
    act = (d.get("act") or 1) - 1
    cv = D.cards()
    return dict(act=act, floor=[8, 25, 42][act], hp=max(1, round(hp_frac * mhp)), max_hp=mhp, gold=100,
                deck=[(cv(c), u) for c, u in deck], relics=[D.relics()(D.START_RELIC)])


def nave_records(path=NAVE_CHECKS):
    """[(kind, record)] scorable records from Nave's notes; the choice is his."""
    if not path.exists():
        return []
    cv = D.cards()
    out = []
    for d in json.loads(path.read_text()):
        s = nave_state(d)
        k = d["kind"]
        known = [c for c, _ in s["deck"]]
        if k == "rest" and d.get("choice") in ("SMITH", "HEAL"):
            opts = [(D.T_HEAL, 0, 0), (D.T_SMITH, 0, 0)]
            out.append(("rest", dict(s, kind="rest", options=opts, choice=int(d["choice"] == "SMITH"))))
        elif k == "upgrade" and d.get("target"):
            t = cv(d["target"])
            alts = [cv(a) for a in d.get("alternatives") or [] if cv(a) > 1 and cv(a) != t]
            if not alts:
                alts = sorted({c for c in known if c != t and D.upgradable(cv.name(c), 0)})
            if t <= 1 or not alts:
                continue
            if t not in known:
                s = dict(s, deck=s["deck"] + [(t, 0)])
            opts = [(D.T_CARD, t, 0)] + [(D.T_CARD, a, 0) for a in dict.fromkeys(alts)]
            out.append(("smith_target", dict(s, kind="smith_target", options=opts, choice=0)))
        elif k == "remove" and d.get("target"):
            t = cv(d["target"])
            cand = sorted({c for c in known if D.removable(cv.name(c))} | {t})
            if t <= 1 or len(cand) < 2:
                continue
            out.append(("shop_remove", dict(s, kind="shop_remove",
                                            options=[(D.T_CARD, c, 0) for c in cand], choice=cand.index(t))))
        elif k == "pick" and d.get("picked"):
            t = cv(d["picked"])
            others = [cv(c) for c in d.get("offered") or [] if cv(c) > 1 and cv(c) != t]
            if t <= 1:
                continue
            opts = [(D.T_CARD, t, 0)] + [(D.T_CARD, c, 0) for c in others] + [(D.T_SKIP, 0, 0)]
            out.append(("card_pick" if others else "card_take", dict(s, kind="card_reward", options=opts,
                                                                       choice=0)))
        elif k == "skip" and d.get("offered"):
            offered = [cv(c) for c in d["offered"] if cv(c) > 1]
            if not offered:
                continue
            opts = [(D.T_CARD, c, 0) for c in offered] + [(D.T_SKIP, 0, 0)]
            out.append(("card_skip", dict(s, kind="card_reward", options=opts, choice=len(offered))))
    return out


def nave_check(net, items):
    by = collections.defaultdict(list)
    chance = collections.defaultdict(list)
    if not items:
        return {}
    p = scores(net, [r for _, r in items])
    for (k, r), q in zip(items, p):
        by[k].append(int(np.argmax(q)) == r["choice"])
        chance[k].append(1 / len(r["options"]))
    return {k: dict(n=len(v), agree=float(np.mean(v)), chance=float(np.mean(chance[k]))) for k, v in by.items()}


def report(args):
    torch.set_num_threads(2)
    data = load_records()
    recs = data["records"]
    held = [r for r in recs if r["heldout"]]
    train_recs = [r for r in recs if not r["heldout"]]
    items = nave_records()
    cv = D.cards()
    out = {}
    for e, w in variants():
        name = model_path(e, w).stem
        net = load_model(model_path(e, w))
        cr = card_ranking(net, train_recs)
        sr = smith_ranking(net, held + train_recs)
        out[name] = dict(rates=rates(net, held), nave=nave_compare(cr, sr), nave_check=nave_check(net, items),
                         top_cards=[cv.name(c) for c in sorted(cr, key=cr.get, reverse=True)[:25]],
                         bottom_cards=[cv.name(c) for c in sorted(cr, key=cr.get)[:10]])
    nave = nave_cards()
    out["nave_tiers"] = {c: t for c, (t, _) in nave.items()}
    (DATA / "report.json").write_text(json.dumps(out, indent=1))
    for name, v in out.items():
        if name == "nave_tiers":
            continue
        print(f"== {name}")
        print(" smith by HP band (human / prior argmax / prior P):",
              {b: (round(x["human"], 3), round(x["prior"], 3), round(x["p"], 3)) for b, x in v["rates"]["smith"].items()})
        for k in ("skip", "removal_use"):
            x = v["rates"].get(k)
            if x:
                print(f" {k}: n {x['n']} human {x['human']:.3f} prior {x['prior']:.3f} P {x['p']:.3f}")
        print(" remove targets:", v["rates"].get("remove_targets"))
        nv = v["nave"]
        print(f" nave: card-pick rank vs tier rho {nv['rho_tier']:.3f} (n {nv['n_tier']}), smith lift vs "
              f"upgrade priority rho {nv['rho_upgrade']:.3f} (humans' {nv['rho_upgrade_human']:.3f}, n "
              f"{nv['n_upgrade']}), smith lift prior vs humans rho {nv['rho_smith_prior_human']:.3f} (n "
              f"{nv['n_smith']}), top-10 overlap with Nave: prior {nv['smith_top10_overlap']}/10, humans "
              f"{nv['human_top10_overlap']}/10")
        print(" smith top 10 prior:", nv["smith_top10"])
        print(" smith top 10 humans:", nv["human_smith_top10"])
        print(" nave top-10 upgrades' rank in prior:", nv["nave_top10_ranks"])
        print(" top cards:", [(c, nave.get(c, (None,))[0]) for c in v["top_cards"]])
        print(" bottom cards:", v["bottom_cards"])
        print(" nave check:", v["nave_check"])


# ---------------------------------------------------------------------------------------------
# Adapter: the same record from the env's run obs

# Action indices (`crates/sts2-rl/src/run_action.rs`): combat 0-139, then the run blocks.
ANCIENT_BASE, EVENT_BASE, CLAIM_BASE, PICK_CARD_BASE = 140, 148, 164, 172
ALTERNATIVE_BASE, SKIP_BASE, REST_BASE, SELECT_BASE = 212, 228, 236, 250
BUY_CARD_BASE, BUY_RELIC_BASE, REMOVE_CARD, PROCEED = 333, 341, 349, 354
PHASE_ANCIENT, PHASE_EVENT, PHASE_REWARDS, PHASE_SELECT, PHASE_REST, PHASE_SHOP = 0, 1, 4, 5, 7, 9
SELECT_SMITH, SELECT_SHOP_REMOVAL = 2, 6
DECK_AT, RELICS_AT, ANCIENT_AT, EVENT_AT, REWARDS_AT, SELECT_AT = 32, 352, 972, 998, 1024, 1192
REST_AT, SHOP_AT = 1648, 1666
REWARD_BYTES, OPT_SLOTS = 21, 8
REST_KIND = {0: D.T_HEAL, 1: D.T_SMITH}  # RestOptionKind index -> token; others are OTHER
# The env's total floor at Neow is 1, as in the file (checked on the 1B pin's runs).
FLOOR_OFFSET = 0


def u16(o, at):
    return int(o[at]) | int(o[at + 1]) << 8


def i16(o, at):
    v = u16(o, at)
    return v - 65536 if v >= 32768 else v


def obs_state(o):
    deck = [(u16(o, DECK_AT + 4 * i), int(o[DECK_AT + 4 * i + 2])) for i in range(80) if u16(o, DECK_AT + 4 * i)]
    rel = [u16(o, RELICS_AT + 4 * i) for i in range(32) if u16(o, RELICS_AT + 4 * i)]
    return dict(act=int(o[2]), floor=int(o[7]) + FLOOR_OFFSET, hp=i16(o, 8), max_hp=i16(o, 10),
                gold=i16(o, 12), deck=deck, relics=rel)


def card_rewards(o):
    """[(reward slot, [(card, upgrade)])] of the rewards screen's card rewards."""
    out = []
    for r in range(8):
        at = REWARDS_AT + r * REWARD_BYTES
        if o[at] == 5:
            cs = [(u16(o, at + 6 + 3 * c), int(o[at + 8 + 3 * c])) for c in range(5)]
            out.append((r, [c for c in cs if c[0]]))
    return out


def obs_records(o, action, shop_seen):
    """The records (with the bot's choice) that action `action` at obs row `o` resolves.

    `shop_seen` is a per-env set the caller resets when the phase leaves the shop: it keeps one
    `shop_removal` record per visit."""
    phase = int(o[3])
    s = obs_state(o)
    out = []
    if phase == PHASE_REWARDS:
        for r, cs in card_rewards(o):
            opts = [(D.T_CARD, c, u) for c, u in cs] + [(D.T_SKIP, 0, 0)]
            lo = PICK_CARD_BASE + 5 * r
            if lo <= action < lo + len(cs):
                ch = action - lo
            elif action == SKIP_BASE + r or action == PROCEED or ALTERNATIVE_BASE + 2 * r <= action < ALTERNATIVE_BASE + 2 * r + 2:
                ch = len(cs)
            else:
                continue
            out.append(dict(s, kind="card_reward", options=opts, choice=ch))
    elif phase == PHASE_REST and REST_BASE <= action < REST_BASE + OPT_SLOTS:
        kinds = [(int(o[REST_AT + 2 * i]) - 1, int(o[REST_AT + 2 * i + 1])) for i in range(OPT_SLOTS)]
        toks = []
        for k, en in kinds:
            if k >= 0 and en:
                t = REST_KIND.get(k, D.T_OTHER)
                if t not in toks:
                    toks.append(t)
        k = kinds[action - REST_BASE][0]
        ch = REST_KIND.get(k, D.T_OTHER)
        if ch in toks and len(toks) > 1:
            out.append(dict(s, kind="rest", options=[(t, 0, 0) for t in toks], choice=toks.index(ch)))
    elif phase == PHASE_SELECT and SELECT_BASE <= action < SELECT_BASE + 80:
        purpose = int(o[SELECT_AT])
        kind = {SELECT_SMITH: "smith_target", SELECT_SHOP_REMOVAL: "shop_remove"}.get(purpose)
        if kind:
            at = SELECT_AT + 8
            cs = [(u16(o, at + 5 * i), int(o[at + 5 * i + 2])) for i in range(80) if u16(o, at + 5 * i)]
            picked = cs[action - SELECT_BASE] if action - SELECT_BASE < len(cs) else None
            uniq = sorted(set(cs))
            if picked and len(uniq) > 1:
                out.append(dict(s, kind=kind, options=[(D.T_CARD, c, u) for c, u in uniq],
                                choice=uniq.index(picked)))
    elif phase == PHASE_SHOP:
        cards = [(i, u16(o, SHOP_AT + 6 * i), int(o[SHOP_AT + 6 * i + 2])) for i in range(8)
                 if u16(o, SHOP_AT + 6 * i)]
        rel_at = SHOP_AT + 48
        relics_ = [(i, u16(o, rel_at + 4 * i)) for i in range(4) if u16(o, rel_at + 4 * i)]
        used = int(o[SHOP_AT + 48 + 32 + 2])
        copts = [(D.T_CARD, c, u) for _, c, u in cards] + [(D.T_NONE, 0, 0)]
        ropts = [(D.T_RELIC, r, 0) for _, r in relics_] + [(D.T_NONE, 0, 0)]
        if BUY_CARD_BASE <= action < BUY_CARD_BASE + 8:
            j = [i for i, _, _ in cards].index(action - BUY_CARD_BASE)
            out.append(dict(s, kind="shop_card", options=copts, choice=j))
        elif BUY_RELIC_BASE <= action < BUY_RELIC_BASE + 4:
            j = [i for i, _ in relics_].index(action - BUY_RELIC_BASE)
            out.append(dict(s, kind="shop_relic", options=ropts, choice=j))
        elif action == REMOVE_CARD and "removal" not in shop_seen:
            shop_seen.add("removal")
            out.append(dict(s, kind="shop_removal", options=[(D.T_REMOVE, 0, 0), (D.T_NONE, 0, 0)], choice=0))
        elif action == PROCEED:
            if cards:
                out.append(dict(s, kind="shop_card", options=copts, choice=len(cards)))
            if relics_:
                out.append(dict(s, kind="shop_relic", options=ropts, choice=len(relics_)))
            if not used and "removal" not in shop_seen:
                out.append(dict(s, kind="shop_removal", options=[(D.T_REMOVE, 0, 0), (D.T_NONE, 0, 0)],
                                choice=1))
            shop_seen.add("removal")
    elif phase == PHASE_ANCIENT and ANCIENT_BASE <= action < ANCIENT_BASE + OPT_SLOTS:
        opts = [(i, u16(o, ANCIENT_AT + 2 + 3 * i)) for i in range(OPT_SLOTS)
                if u16(o, ANCIENT_AT + 2 + 3 * i) and not o[ANCIENT_AT + 4 + 3 * i]]
        idx = [i for i, _ in opts]
        if action - ANCIENT_BASE in idx and len(opts) > 1:
            out.append(dict(s, kind="ancient", options=[(D.T_RELIC, r, 0) for _, r in opts],
                            choice=idx.index(action - ANCIENT_BASE)))
    elif phase == PHASE_EVENT and EVENT_BASE <= action < EVENT_BASE + OPT_SLOTS:
        opts = [(i, u16(o, EVENT_AT + 2 + 3 * i)) for i in range(OPT_SLOTS)
                if u16(o, EVENT_AT + 2 + 3 * i) and o[EVENT_AT + 4 + 3 * i]]
        idx = [i for i, _ in opts]
        if action - EVENT_BASE in idx and len(opts) > 1:
            out.append(dict(s, kind="event", options=[(D.T_EVENT, k, 0) for _, k in opts],
                            choice=idx.index(action - EVENT_BASE)))
    return out


def adapter(args):
    """Greedy runs of the pin; every run-level decision as a prior record with the bot's choice."""
    torch.set_num_threads(2)
    from sts2 import HOLD
    import e2e_eval as V
    import rln40_smith as S

    device = torch.device(args.device)
    pol = V.load_policy(args.ckpt, args.weights, device)
    first, _ = V.PANELS["DEV-1000"]
    seeds = list(range(first, first + args.runs))
    env, flags = S.make_env(len(seeds), seeds, pol, args.threads)
    n = len(seeds)
    done = np.zeros(n, dtype=bool)
    shop_seen = [set() for _ in range(n)]
    recs, neow_floor = [], []
    try:
        while not done.all():
            live = np.nonzero(~done)[0]
            logits, _ = S.forward(pol, env, live, flags, device)
            act = logits.argmax(-1).cpu().numpy().astype(np.uint16)
            for j, i in enumerate(live):
                o = env.obs[i]
                if int(o[3]) != PHASE_SHOP:
                    shop_seen[i].clear()
                if int(o[3]) == PHASE_ANCIENT and int(o[2]) == 0:
                    neow_floor.append(int(o[7]))
                for r in obs_records(o, int(act[j]), shop_seen[i]):
                    recs.append(dict(r, seed=seeds[i]))
            step = np.full(n, HOLD, dtype=np.uint16)
            step[live] = act
            env.step(step)
            for i in live[env.done[live].astype(bool)]:
                done[i] = True
    finally:
        env.close()
    out = DATA / "adapter_records.pkl.gz"
    with gzip.open(out, "wb") as f:
        pickle.dump(dict(records=recs, neow_floor=neow_floor), f)
    print(f"{n} runs, {len(recs)} decisions; env floor at Neow: {collections.Counter(neow_floor)} -> {out}")
    disagree(args)


def disagree(args):
    with gzip.open(DATA / "adapter_records.pkl.gz", "rb") as f:
        recs = pickle.load(f)["records"]
    out = {}
    for e, w in variants():
        net = load_model(model_path(e, w))
        p = scores(net, recs)
        by = collections.defaultdict(list)
        for r, q in zip(recs, p):
            by[r["kind"]].append(int(np.argmax(q)) != r["choice"])
        res = {k: dict(n=len(v), disagree=float(np.mean(v))) for k, v in by.items()}
        res["all"] = dict(n=len(recs), disagree=float(np.mean([x for v in by.values() for x in v])))
        res["rates_bot_vs_prior"] = bot_rates(recs, p)
        out[model_path(e, w).stem] = res
        print(f"== {model_path(e, w).stem}: bot vs prior disagreement")
        for k, v in res.items():
            print(f"  {k}: {v}")
    (DATA / "adapter.json").write_text(json.dumps(out, indent=1))


def bot_rates(recs, p):
    """Smith (rest visits with both), skip and removal use: the bot's choices vs the prior's argmax."""
    out = {}
    rest = [(r, q) for r, q in zip(recs, p) if r["kind"] == "rest"
            and {D.T_SMITH, D.T_HEAL} <= {t for t, _, _ in r["options"]}]
    if rest:
        si = [[t for t, _, _ in r["options"]].index(D.T_SMITH) for r, _ in rest]
        out["smith"] = dict(n=len(rest), bot=float(np.mean([r["choice"] == s for (r, _), s in zip(rest, si)])),
                            prior=float(np.mean([int(np.argmax(q)) == s for (_, q), s in zip(rest, si)])))
    cr = [(r, q) for r, q in zip(recs, p) if r["kind"] == "card_reward"]
    if cr:
        out["skip"] = dict(n=len(cr), bot=float(np.mean([r["choice"] == len(r["options"]) - 1 for r, _ in cr])),
                           prior=float(np.mean([int(np.argmax(q)) == len(r["options"]) - 1 for r, q in cr])))
    su = [(r, q) for r, q in zip(recs, p) if r["kind"] == "shop_removal"]
    if su:
        out["removal_use"] = dict(n=len(su), bot=float(np.mean([r["choice"] == 0 for r, _ in su])),
                                  prior=float(np.mean([int(np.argmax(q)) == 0 for _, q in su])))
    return out


def main(argv=None):
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    t = sub.add_parser("train")
    t.add_argument("--expert", action="store_true")
    t.add_argument("--wins-only", action="store_true")
    t.add_argument("--epochs", type=int, default=12)
    t.add_argument("--batch", type=int, default=256)
    t.add_argument("--lr", type=float, default=2e-3)
    e = sub.add_parser("eval")
    e.add_argument("--heldout", action="store_true", help="score the held-out players (the only mode)")
    sub.add_parser("report")
    a = sub.add_parser("adapter")
    a.add_argument("ckpt")
    a.add_argument("--weights", default="ema0.999")
    a.add_argument("--runs", type=int, default=40)
    a.add_argument("--threads", type=int, default=2)
    a.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    sub.add_parser("disagree")
    args = ap.parse_args(argv)
    {"train": train, "eval": evaluate, "report": report, "adapter": adapter, "disagree": disagree}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
