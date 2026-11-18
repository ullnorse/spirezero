"""RLN44 profile: where one update minibatch and one collect go on the GPU (`torch.profiler`).

Builds a `train_e2e.Learner` on a config (default NECRO5's shape, `--config`), runs `--warm`
whole iterations (every compiled shape, then timing excluded), then profiles:

* one update minibatch (the profiler steps at each optimiser step: the 3rd minibatch of the
  next update is recorded);
* one collect (all its steps; per-step numbers are the totals / steps).

For each it prints the wall, the GPU-busy share (the union of the kernel intervals over the
wall) and the top CUDA kernels and CPU ops by self time. Writes nothing outside `--out`.

    STS2_FFI_LIB=target/ffi/libsts2_ffi.so python python/rln44_bench.py \\
        --config target/rln44/base.json --out target/rln44/prof
"""

import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import train_e2e as T  # noqa: E402

import torch  # noqa: E402
from torch.profiler import ProfilerActivity, profile, schedule  # noqa: E402


def busy(trace_path):
    """(wall ms of the profiled step, GPU-busy share, kernel count) from a chrome trace."""
    ev = json.loads(pathlib.Path(trace_path).read_text())["traceEvents"]
    steps = [e for e in ev if e.get("name", "").startswith("ProfilerStep#") and e.get("ph") == "X"]
    if steps:
        lo = min(e["ts"] for e in steps)
        hi = max(e["ts"] + e["dur"] for e in steps)
    else:
        cpu = [e for e in ev if e.get("ph") == "X" and e.get("cat") == "cpu_op"]
        lo, hi = min(e["ts"] for e in cpu), max(e["ts"] + e["dur"] for e in cpu)
    ks = sorted((e["ts"], e["ts"] + e["dur"]) for e in ev
                if e.get("ph") == "X" and e.get("cat") in ("kernel", "gpu_memcpy", "gpu_memset"))
    ks = [(max(a, lo), min(b, hi)) for a, b in ks if b > lo and a < hi]
    total, cur_a, cur_b = 0.0, None, None
    for a, b in ks:
        if cur_b is None or a > cur_b:
            if cur_b is not None:
                total += cur_b - cur_a
            cur_a, cur_b = a, b
        else:
            cur_b = max(cur_b, b)
    if cur_b is not None:
        total += cur_b - cur_a
    return (hi - lo) / 1e3, total / (hi - lo), len(ks)


def top(prof, n):
    ka = prof.key_averages()
    gpu = sorted(ka, key=lambda e: e.self_device_time_total, reverse=True)[:n]
    cpu = sorted(ka, key=lambda e: e.self_cpu_time_total, reverse=True)[:n]
    return ([(e.key[:70], round(e.self_device_time_total / 1e3, 2), e.count) for e in gpu],
            [(e.key[:70], round(e.self_cpu_time_total / 1e3, 2), e.count) for e in cpu])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--set", action="append", default=[])
    ap.add_argument("--warm", type=int, default=2)
    ap.add_argument("--top", type=int, default=15)
    ap.add_argument("--shapes", action="store_true", help="record shapes and stacks (update)")
    ap.add_argument("--no-collect", action="store_true")
    args = ap.parse_args()
    out = pathlib.Path(args.out)
    over = json.loads(pathlib.Path(args.config).read_text())
    for kv in args.set:
        k, v = kv.split("=", 1)
        over[k] = json.loads(v)
    cfg = T.Config.from_dict({**T.dataclasses.asdict(T.Config()), **over})
    L = T.Learner(cfg, out / "run")
    for _ in range(args.warm):
        rec = L.iterate()
        print(f"warm it {rec['iteration']} dps {rec['collect_dps']:.0f}/{rec['update_dps']:.0f}/"
              f"{rec['total_dps']:.0f}", flush=True)
    res = {}
    # One update minibatch: step the profiler at each optimiser step.
    acts = [ProfilerActivity.CPU, ProfilerActivity.CUDA]
    tr = out / "update.json"
    with profile(activities=acts, record_shapes=args.shapes, schedule=schedule(wait=1, warmup=1, active=1, repeat=1),
                 on_trace_ready=lambda p: p.export_chrome_trace(str(tr))) as prof:
        step0 = L.opt.step

        def stepped(*a, **k):
            r = step0(*a, **k)
            prof.step()
            return r
        L.opt.step = stepped
        buf, runs = L.collect()
        L.update(buf)
        L.opt.step = step0
    wall, share, nk = busy(tr)
    g, c = top(prof, args.top)
    res["update_minibatch"] = dict(wall_ms=round(wall, 1), gpu_busy=round(share, 3), kernels=nk,
                                   top_gpu=g, top_cpu=c)
    if args.shapes:
        for e in sorted(prof.key_averages(group_by_input_shape=True),
                        key=lambda e: e.self_device_time_total, reverse=True)[:25]:
            print(f"{e.self_device_time_total / 1e3:8.2f} ms {e.count:4d} {e.key[:40]} "
                  f"{str(e.input_shapes)[:160]}")
    if args.no_collect:
        args_done(res, out)
        L.close()
        return
    # One collect.
    tr = out / "collect.json"
    with profile(activities=acts) as prof:
        L.collect()
        torch.cuda.synchronize()
    prof.export_chrome_trace(str(tr))
    wall, share, nk = busy(tr)
    g, c = top(prof, args.top)
    res["collect"] = dict(wall_ms=round(wall, 1), per_step_ms=round(wall / cfg.steps, 2),
                          gpu_busy=round(share, 3), kernels=nk, top_gpu=g, top_cpu=c)
    args_done(res, out)
    L.close()


def args_done(res, out):
    (out / "profile.json").write_text(json.dumps(res, indent=1))
    for k, r in res.items():
        print(f"== {k}: wall {r['wall_ms']} ms, GPU busy {r['gpu_busy']:.1%}, kernels {r['kernels']}")
        print("  top GPU:", *r["top_gpu"], sep="\n    ")
        print("  top CPU:", *r["top_cpu"], sep="\n    ")


if __name__ == "__main__":
    main()
