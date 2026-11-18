"""WEB82: the CB3 combat net for the web client, and its parity rows.

  export CKPT OUT.bin            the policy's weights (EMA 0.996 by default) as one file the bot's
                                 wasm reads (`crates/sts2-web-bot/src/net.rs`): `CB3N`, a u32
                                 header length, a JSON header {name: [offset, shape]}, then
                                 little-endian f32 data. The value module and the MAPNODE stream
                                 are left out (combat rows never use them); the ID table and the
                                 content-stats rows keep only the vocabularies a fight reads.
  dump CKPT BENCH OUT.bin        plays the bench's boss and elite fights greedily (PyTorch, CPU)
                                 and keeps up to --rows combat decisions: obs, mask words and the
                                 masked logits, for `cargo test -p sts2-web-bot --test parity`.

CPU only (`CUDA_VISIBLE_DEVICES=` is set before torch loads)."""

import argparse
import json
import os
import pathlib
import struct
import sys

os.environ["CUDA_VISIBLE_DEVICES"] = ""

import numpy as np  # noqa: E402
import torch  # noqa: E402

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

ROOT = pathlib.Path(__file__).resolve().parents[1]
# The ID-table vocabularies a fight's tokens read (`encode._sections`), shipped as row ranges.
FIGHT_TABLES = ("cards", "relics", "monsters", "powers", "potions", "encounters", "events",
                "ancients")
VOCAB_FILE = dict(cards="cards", relics="relics", monsters="monsters", powers="powers",
                  potions="potions", encounters="encounters", events="events",
                  ancients="ancients")


def load(ckpt, weights):
    """The checkpoint's net with `weights` (raw or `ema<decay>`), on the CPU (`e2e_eval.load_policy`,
    tolerating config keys this checkout's `Config` lacks, such as CB3's `lr_sched`)."""
    import dataclasses

    import train_e2e as T

    state = torch.load(ckpt, map_location="cpu", weights_only=False)
    known = {f.name for f in dataclasses.fields(T.Config)}
    cfg = T.Config.from_dict({k: v for k, v in state["config"].items() if k in known})
    net = T.net_of(cfg)
    net.load_net(state["net"])
    net.env_opts = dict(discard_potion=cfg.discard_potion, plan_depth=cfg.plan_depth)
    if weights != "raw":
        decay = float(weights[3:])
        k = [i for i, d in enumerate(cfg.ema) if abs(d - decay) < 1e-12][0]
        with torch.no_grad():
            for p, t in zip(net.parameters(), state["emas"][k]):
                p.copy_(t)
    return net.eval(), cfg


def vocab_rows():
    """[(table, ID-row start, rows)] of `FIGHT_TABLES`: the vocabulary's ids (0 empty, 1 unknown,
    then one per line) at the table's offset."""
    from sts2.e2e import encode as E

    out = []
    for t in FIGHT_TABLES:
        lines = (ROOT / "crates/sts2-rl/vocab" / f"{VOCAB_FILE[t]}.txt").read_text().splitlines()
        out.append((t, E._OFF[t], len(lines) + 2))
    return out


def cmd_export(a):
    from sts2.e2e import encode as E

    net, cfg = load(a.ckpt, a.weights)
    assert cfg.obs_v2 and cfg.obs_full and cfg.card_stats and cfg.discard_potion
    assert not cfg.obs_events and not cfg.run_head and cfg.layers == 6 and cfg.d == 256
    sd = {k: v.float() for k, v in net.state_dict().items()}
    ranges = vocab_rows()
    pick = torch.cat([torch.arange(s, s + n) for _, s, n in ranges])
    with torch.no_grad():
        stats = net.cstats.rows().float()  # (2 * ID_ROWS, d)
    keep = {}
    keep["ids"] = sd["ids.weight"][pick]
    keep["stats_base"] = stats[pick]
    keep["stats_up"] = stats[E.ID_ROWS + pick]
    for k, v in sd.items():
        if k.startswith(("v_", "cstats.", "ids.")) or k in ("x_v2.node", "x_v2.gate") \
                or k.startswith("x_v2.ystem"):
            continue
        keep[k] = v
    header, blobs, at = {}, [], 0
    for k, v in keep.items():
        arr = v.contiguous().numpy().astype("<f4")
        header[k] = [at, list(arr.shape)]
        blobs.append(arr.tobytes())
        at += arr.size
    header["_id_ranges"] = [[s, n] for _, s, n in ranges]
    header["_source"] = [str(a.ckpt), a.weights]
    hj = json.dumps(header).encode()
    out = pathlib.Path(a.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with open(out, "wb") as f:
        f.write(b"CB3N")
        f.write(struct.pack("<I", len(hj)))
        f.write(hj)
        f.write(b"".join(blobs))
    print(f"{out}: {out.stat().st_size / 1e6:.1f} MB, {at} floats, {len(keep)} tensors")


def cmd_dump(a):
    import naz3_combat as C

    net, _ = load(a.ckpt, a.weights)
    net.env_opts = dict(net.env_opts)
    torch.set_num_threads(a.threads)
    recs = C.read_jsonl(pathlib.Path(a.bench).expanduser())
    recs = [r for r in recs if C.room_of(r) in ("boss", "elite")]
    rng = np.random.default_rng(0)
    recs = [recs[i] for i in sorted(rng.choice(len(recs), a.fights, replace=False))]
    rows = []
    every = a.every

    def on_step(env, idx, k_of, lg, act):
        for j, i in enumerate(idx):
            if len(rows) >= a.rows or env.obs[i, 3] != 3 or (int(k_of[j]) + len(rows)) % every:
                continue
            rows.append((env.obs[i].copy(), env.masks[i].copy(),
                         lg[j].float().cpu().numpy().copy(), int(act[j])))

    res = C.play(net, torch.device("cpu"), recs, 1, 7_000_000, a.fights, None, a.threads,
                 on_step=on_step)
    print(json.dumps(dict(fights=len(res["died"]), died=float(res["died"].mean()),
                          rows=len(rows))))
    width = len(rows[0][0])
    with open(a.out, "wb") as f:
        f.write(b"CB3P")
        f.write(struct.pack("<III", len(rows), width, len(rows[0][2])))
        for obs, mask, lg, act in rows:
            f.write(obs.astype(np.uint8).tobytes())
            f.write(mask.view(np.uint64).astype("<u8").tobytes())
            f.write(np.where(np.isfinite(lg), lg, -1e30).astype("<f4").tobytes())
            f.write(struct.pack("<I", act))
    print(f"{a.out}: {len(rows)} rows of {width} bytes")


def main():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    e = sub.add_parser("export")
    e.add_argument("ckpt")
    e.add_argument("out")
    e.add_argument("--weights", default="ema0.996")
    d = sub.add_parser("dump")
    d.add_argument("ckpt")
    d.add_argument("bench")
    d.add_argument("out")
    d.add_argument("--weights", default="ema0.996")
    d.add_argument("--fights", type=int, default=48)
    d.add_argument("--rows", type=int, default=800)
    d.add_argument("--every", type=int, default=3, help="keep every n-th combat decision")
    d.add_argument("--threads", type=int, default=4)
    a = p.parse_args()
    return cmd_export(a) if a.cmd == "export" else cmd_dump(a)


if __name__ == "__main__":
    sys.exit(main())
