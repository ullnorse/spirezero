"""Shared loading for VN1 (the value net from the bot's own outcomes): `value-collect` data as
raw observation rows, outcomes, fight ids, encounters and turn numbers."""

import pathlib
import sys

import numpy as np
import torch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import combat_net  # noqa: E402

W = combat_net.OBS_BYTES + combat_net.STATIC_BYTES
VOCAB = pathlib.Path(__file__).resolve().parent.parent / "crates/sts2-rl/vocab"


def encounters():
    """Encounter vocabulary id -> name."""
    return dict(enumerate((VOCAB / "encounters.txt").read_text().split("\n")))


def load(d, extra=None):
    """`d`'s rows: raw (N × W uint8), table (seed, kind, won, hp, max hp), fight ids, the
    encounter id and turn of each row. `extra` names a per-row f32 file to load alongside."""
    d = pathlib.Path(d)
    t = np.loadtxt(d / "samples.tsv", dtype=np.int64, ndmin=2)
    o = np.fromfile(d / "obs.bin", np.uint8)
    n = min(len(o) // W, len(t))
    raw, t = o[: n * W].reshape(n, W), t[:n]
    enc = raw[:, combat_net.OBS_BYTES + 4].astype(np.int64) + raw[:, combat_net.OBS_BYTES + 5].astype(np.int64) * 256
    turn = raw[:, 12].astype(np.int64) + raw[:, 13].astype(np.int64) * 256
    new = np.ones(n, bool)
    new[1:] = (t[1:, 0] != t[:-1, 0]) | (enc[1:] != enc[:-1]) | (turn[1:] <= turn[:-1])
    fight = np.cumsum(new) - 1
    return raw, t, fight, enc, turn


def targets(t):
    win = t[:, 2].astype(np.float32)
    hp = np.clip(t[:, 3] / np.maximum(t[:, 4], 1), 0, 1).astype(np.float32) * win
    return win, hp


@torch.inference_mode()
def heads(net, raw, bs=16384):
    out = [net(raw[i:i + bs]) for i in range(0, len(raw), bs)]
    return tuple(torch.cat([o[k] for o in out]) for k in range(3))


def load_net(path, device):
    saved = torch.load(path, map_location=device, weights_only=False)
    net = combat_net.Net(saved["hidden"]).to(device)
    net.load_state_dict(saved["model"])
    net.eval()
    return net, saved


def auc(score, label):
    """Mann-Whitney AUC (NaN without both classes)."""
    score, label = np.asarray(score, np.float64), np.asarray(label, bool)
    pos, neg = label.sum(), (~label).sum()
    if pos == 0 or neg == 0:
        return float("nan")
    _, inv, cnt = np.unique(score, return_inverse=True, return_counts=True)
    r = (np.cumsum(cnt) - cnt)[inv] + (cnt[inv] + 1) / 2.0  # average ranks over ties
    return float((r[label].sum() - pos * (pos + 1) / 2) / (pos * neg))
