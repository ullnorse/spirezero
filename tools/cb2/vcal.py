"""usage: vcal.py DATA_DIR CKPT...  -- value calibration on held-out search data: for each
checkpoint, the PPO value head's MSE to the row's actual return (`naz3_ei train`'s CB1 target: win
bonus 1 plus hp_coef x the HP fraction still to change) and the mean value vs mean return in five
bins of the prediction. Only searched rows of finished fights are used."""
import sys

import numpy as np

sys.path.insert(0, "python")
import naz3_combat as C  # noqa: E402
import naz3_ei as N  # noqa: E402

HP_COEF = 0.3
ROWS = 60_000


def main():
    import torch

    import rln36_boss as B
    from sts2.e2e import encode as E

    data, cks = sys.argv[1], sys.argv[2:]
    obs, masks, _, _, won, hpf, srch, priv = N.load_data([data], 0.03)
    keep = np.nonzero(srch)[0]
    keep = np.random.default_rng(0).permutation(keep)[:ROWS]
    obs, masks, won, hpf, priv = obs[keep], masks[keep], won[keep], hpf[keep], priv[keep]
    phi0 = np.maximum(0, C.i16_col(obs, C.H_HP)) / np.maximum(1, C.i16_col(obs, C.H_MAXHP))
    ret = won + HP_COEF * (np.where(won > 0, hpf, 0.0) - phi0)
    for ck in cks:
        net, dev = C.net_and_dev(ck, "raw")
        f = B.flags(net)
        out = []
        with torch.no_grad():
            for k in range(0, len(obs), 1024):
                sl = slice(k, k + 1024)
                o = torch.from_numpy(obs[sl]).to(dev)
                mk = torch.from_numpy(masks[sl].view(np.int64)).to(dev)
                tok = E.encode(o, mk, **f)
                with torch.autocast(dev.type, dtype=torch.bfloat16, enabled=dev.type == "cuda"):
                    _, h, valid = net.policy(tok)
                    v = net.value(h, valid, E.encode_priv(torch.from_numpy(priv[sl]).to(dev)),
                                  tok["combat"])
                out.append(v.float().cpu().numpy())
        v = np.concatenate(out)
        edges = np.quantile(v, [0.2, 0.4, 0.6, 0.8])
        b = np.digitize(v, edges)
        bins = " ".join(f"{v[b == i].mean():.2f}/{ret[b == i].mean():.2f}" for i in range(5))
        print(f"{ck}\tmse {np.mean((v - ret) ** 2):.4f}\tbias {np.mean(v - ret):+.4f}\t"
              f"bins v/ret {bins}\trows {len(v)}", flush=True)
        del net
        torch.cuda.empty_cache()


if __name__ == "__main__":
    main()
