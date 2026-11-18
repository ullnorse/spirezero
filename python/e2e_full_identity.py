"""RLN30/RLN33: with `obs_full` and `obs_events` off, a checkpoint gives the same bits as before.

`python e2e_full_identity.py STATE.pt --ref-python DIR --ref-lib LIB [--states N] [--runs N]`

The reference is the code before RLN30 (`DIR`: a `python/` tree of that commit, e.g. `git
archive main python`) with its FFI library (`LIB`, e.g. a lane's frozen `lib/libsts2_ffi.so`);
the candidate is this tree with its own library (`STS2_FFI_LIB`), which writes the full and event
sections and reads them only with their flags on. Each side, in its own process, on the CPU in fp32:

- draws the honesty SMOKE states with the v2 section (the candidate's rows are wider; the bytes
  before `FULL_AT` must be the reference's rows, and the masks and privileged bytes the same);
- loads the checkpoint's net as a resume and an eval do (`train_e2e.net_of` of its config, then
  the raw weights and each EMA) and records every logit and value;
- plays `--runs` greedy runs with `e2e_eval.play_batch` (raw weights): each run's record.

Exit status 0 when the rows, logits, values and run records are all the same.
"""

import argparse
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent


def dump(state, states, runs, out):
    """One side's rows, outputs and run records (the code on `sys.path`) into `out`."""
    import torch

    import e2e_eval
    import train_e2e as T
    from sts2.e2e import encode as E
    from sts2.e2e.honesty import smoke_states

    torch.set_num_threads(4)
    s = torch.load(state, map_location="cpu", weights_only=False)
    cfg = s["config"]
    obs, masks, priv, _ = smoke_states(states, obs_v2=True)
    net = T.net_of(cfg)
    net.load_state_dict(s["net"])
    net.eval()
    sets = [("raw", [p.detach().clone() for p in net.parameters()])]
    sets += [(f"ema{d}", w) for d, w in zip(cfg["ema"], s["emas"], strict=True)]
    outputs = {}
    with torch.no_grad():
        for name, weights in sets:
            for p, w in zip(net.parameters(), weights, strict=True):
                p.copy_(w)
            ls, vs = [], []
            for i in range(0, len(obs), 250):
                o, m, p = obs[i:i + 250], masks[i:i + 250], priv[i:i + 250]
                logits, value = net(E.encode(o, m, v2=True), E.encode_priv(p))
                ls.append(logits)
                vs.append(value)
            outputs[name] = (torch.cat(ls), torch.cat(vs))
        for p, w in zip(net.parameters(), sets[0][1], strict=True):
            p.copy_(w)
    seeds = list(range(300_000_000, 300_000_000 + runs))
    records = e2e_eval.play_batch(net, seeds, torch.device("cpu"), 4)
    torch.save(dict(obs=obs, masks=masks, priv=priv, outputs=outputs, records=records), out)


def _build_id(lib):
    import ctypes

    handle = ctypes.CDLL(str(lib))
    handle.sts2_build_id.restype = ctypes.c_char_p
    return handle.sts2_build_id().decode()


def _canon(records):
    return json.dumps(records, sort_keys=True,
                      default=lambda x: x.tolist() if hasattr(x, "tolist") else str(x))


def _side(side, py, lib, args):
    """Runs one side's `dump` in its own process (`py` its tree, `lib` its library)."""
    import torch

    scratch = pathlib.Path(tempfile.mkdtemp(dir=HERE.parent / "target"))
    out = scratch / f"{side}.pt"
    try:
        env = dict(os.environ, STS2_FFI_LIB=str(lib), CUDA_VISIBLE_DEVICES="")
        env.pop("STS2_FFI_BUILD_ID", None)
        if side == "ref":  # a frozen lane library, as `tools/e2e_lane.sh` runs it
            env["STS2_FFI_BUILD_ID"] = _build_id(lib)
        subprocess.run([sys.executable, str(HERE / "e2e_full_identity.py"),
                        str(pathlib.Path(args.state).resolve()),
                        "--states", str(args.states), "--runs", str(args.runs),
                        "--dump", str(out)], cwd=py, env=env, check=True)
        return torch.load(out, weights_only=False)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("state")
    ap.add_argument("--ref-python", type=pathlib.Path)
    ap.add_argument("--ref-lib", type=pathlib.Path)
    ap.add_argument("--states", type=int, default=2000)
    ap.add_argument("--runs", type=int, default=32)
    ap.add_argument("--dump", type=pathlib.Path, help=argparse.SUPPRESS)
    args = ap.parse_args(argv)
    if args.dump:
        sys.path[0] = os.getcwd()  # the side's own tree, not this file's
        dump(args.state, args.states, args.runs, args.dump)
        return 0
    import torch

    from sts2.e2e import encode as E

    sides = {}
    for side, py, lib in (("ref", args.ref_python.resolve(), args.ref_lib.resolve()),
                          ("new", HERE, pathlib.Path(os.environ.get("STS2_FFI_LIB", "")))):
        sides[side] = _side(side, py, lib, args)

    a, b = sides["ref"], sides["new"]
    rep = dict(checkpoint=args.state, rows=len(a["obs"]),
               ref_width=a["obs"].shape[1], new_width=b["obs"].shape[1],
               rows_same=bool(torch.equal(a["obs"], b["obs"][:, :a["obs"].shape[1]])),
               rows_with_full_bytes=int((b["obs"][:, E.FULL_AT:] != 0).any(1).sum()),
               rows_with_event_bytes=int((b["obs"][:, E.EVENTS_AT:] != 0).any(1).sum()),
               masks_same=bool(torch.equal(a["masks"], b["masks"])),
               priv_same=bool(torch.equal(a["priv"], b["priv"])), weights={})
    for name, (l1, v1) in a["outputs"].items():
        l2, v2 = b["outputs"][name]
        rep["weights"][name] = dict(rows_with_other_logits=int((l1 != l2).any(1).sum()),
                                    rows_with_other_value=int((v1 != v2).sum()))
    rep["runs"] = len(a["records"])
    rep["runs_same"] = _canon(a["records"]) == _canon(b["records"])
    rep["ok"] = (rep["rows_same"] and rep["masks_same"] and rep["priv_same"] and rep["runs_same"]
                 and a["obs"].shape[1] in (E.OBS_BYTES_V2, E.OBS_BYTES_FULL)
                 and b["obs"].shape[1] == E.OBS_BYTES_EVENTS
                 and all(w["rows_with_other_logits"] == 0 and w["rows_with_other_value"] == 0
                         for w in rep["weights"].values()))
    print(json.dumps(rep))
    return 0 if rep["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
