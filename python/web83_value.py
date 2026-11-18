"""WEB83: what CB3's value head says at turns where the net had a lethal line and did not take it.

  python python/web83_value.py CKPT ROWS.bin [--weights ema0.996]

ROWS.bin is written by `WEB83_LETHAL=1 cargo test --release -p sts2-web-bot --test e2e_parity`
(`target/web83-lethal-{gym,web}.bin`): `W83L`, u32 rows, obs width, priv width, then per row the
missed turn's (obs, 6 mask words, priv), the next turn's (same), and the line's damage margin
(i32). Prints V at the missed turn and at the next turn (combat output). Killing now ends the
fight: a gym fight's return is then `win_bonus` (CB3: 1.0) plus the HP potential's change (0 for
a kill this turn), so V well below that at the missed turn means the value head does not see the
kill. CPU only."""

import argparse
import os
import pathlib
import struct
import sys

os.environ["CUDA_VISIBLE_DEVICES"] = ""

import numpy as np  # noqa: E402
import torch  # noqa: E402

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))


def main():
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("ckpt")
    p.add_argument("rows")
    p.add_argument("--weights", default="ema0.996")
    p.add_argument("--show", type=int, default=12)
    a = p.parse_args()

    import web82_export as W
    from sts2.e2e import encode as E

    net, cfg = W.load(a.ckpt, a.weights)
    torch.set_num_threads(4)
    b = pathlib.Path(a.rows).read_bytes()
    assert b[:4] == b"W83L"
    n, ow, pw = struct.unpack("<III", b[4:16])
    rw = ow + 48 + pw
    stride = 2 * rw + 4
    if n == 0:
        print("no rows")
        return
    raw = np.frombuffer(b[16:16 + n * stride], dtype=np.uint8).reshape(n, stride)

    def value(part):
        r = raw[:, part * rw:(part + 1) * rw]
        obs = torch.from_numpy(r[:, :ow].copy())
        masks = torch.from_numpy(r[:, ow:ow + 48].copy().view(np.int64))
        priv = torch.from_numpy(r[:, ow + 48:].copy())
        with torch.no_grad():
            tok = E.encode(obs, masks, v2=cfg.obs_v2, full=cfg.obs_full, events=cfg.obs_events,
                           discard=cfg.discard_potion)
            ptok = E.encode_priv(priv)
            _, v = net(tok, ptok)
        return v.float().numpy()

    margin = raw[:, 2 * rw:].copy().view("<i4")[:, 0]
    v_now, v_next = value(0), value(1)
    print(f"{n} missed turns: V at the missed turn mean {v_now.mean():.3f} "
          f"(p10 {np.percentile(v_now, 10):.3f}, p90 {np.percentile(v_now, 90):.3f}); "
          f"V a turn later mean {v_next.mean():.3f}; V later - V now mean "
          f"{(v_next - v_now).mean():+.3f}")
    for k in range(min(n, a.show)):
        print(f"  margin {margin[k]:4d}  V now {v_now[k]:.3f}  V next turn {v_next[k]:.3f}")


if __name__ == "__main__":
    sys.exit(main())
