# RLN44: pure PPO, faster learner

Brief: `tasks/RLN44-fast-ppo.md`. Research input: `~/spirebot/target/rln44-research/report.md` (local).
FFI build c7fe421e39c7abbd (main, unchanged: no Rust change). **NECRO5 was not started** (owner,
2026-11-12: an independent review comes first); its command line is in §7.

## 1. Result

Same workload (the NECRO4 config without `init_from`/`prior`, plus `behaviour=true`; from
scratch, the same seeds), iterations 9–14 medians (each run's shapes warmed; `torch.compile`
recompiles stop by iteration 8), decisions/s:

| change (cumulative) | collect | update | total |
|---|---:|---:|---:|
| base (main: compile, E=2, fixed + batch probe), A | 25,039 | 5,270 | 4,349 |
| + `fast_gather` | 25,017 | 7,789 | 5,939 |
| + `epochs=1`, `fixed_probe=false` | 25,432 | 16,463 | 9,993 |
| + `static_head`, `pinned_copies` | 26,355 | 16,533 | 10,116 |
| + `compile_encode` | 33,274 | 16,613 | 11,069 |
| + `compile_net` (all), B | 45,364 | 24,041 | **15,614** |

ABAB: base 4,349 / 4,347, all 15,614 / 15,649 (×3.6). Tokens per row grow as runs get
longer: at iterations 24–30 (30 tokens) base 4,233, all 14,960 (×3.5); at 33 tokens (iterations
80–91 of the all-flags run) all 13,747. **So ≥15k holds up to ~28 tokens; at NECRO4's 30–35 it
is 13.7–15k.** Peak GPU memory 8.1 GB (base 8.9 GB). micro 2048 measured no faster (15,511 vs
15,561) and used 11.1 GB, so micro stays 1,024.

What limits it now (profile, `python/rln44_bench.py`, after all flags):
- **Update:** one minibatch 332 ms (base 714 ms), the GPU 98% busy before and after. It is
  compute: the six blocks' compiled graphs, then embedding backwards (the ID table, the full
  view's `embedding_bag`, ~20%). CUDA graphs cannot help a busy GPU.
- **Collect:** 3.46 s (base 5.09 s), GPU 56% busy: the Rust step (~6.5 ms per step) and the host
  work run while the GPU waits. Double-buffered env halves would hide that (~1–1.5 s per
  iteration) but need the env split into two step groups (seeds, practice pool, resume replay);
  not done. Overlap (deliverable 5) cannot gain more than the collect's idle share, since the
  update keeps the GPU busy; not done.

## 2. Profile (deliverable 1)

`torch.profiler` at the NECRO4 shape, after 2 warm iterations:

| | wall | GPU busy | kernels |
|---|---:|---:|---:|
| update minibatch, base | 714 ms | 98.1% | 9,958 |
| update minibatch, all | 332 ms | 98.3% | 10,149 |
| collect (64 steps), base | 5,087 ms | 53.4% | 260,030 |
| collect, all | 3,457 ms | 55.6% | 132,050 |

The base update was GPU-bound, not sync-bound (the report's estimate). **46% of each minibatch
(330 of 720 ms) was one kernel**: `indexing_backward_kernel`, the backward of
`cstats.rows()[up * ID_ROWS + ident]` (net.py `trunk`). Padding and common cards repeat the same
index thousands of times, and the advanced-index backward serialises them. `F.embedding` does
the same forward with a sorted-segment backward (`fast_gather`). That set the order: gather fix
first, then E=1 (half the update), then the collect.

The collect per step: encode 14.3 ms (2,355 kernels, GPU 33% busy: launch-bound), net 34.6 ms
(3 buckets, 10 syncs per step, 88% busy), env step 6.5 ms. Compiling `encode` whole takes it to
2.5 ms; compiling the net whole (embeddings, blocks, head, value, one graph) removes the eager
adds, copies and gathers that were ~1 s of the collect's GPU time.

## 3. The flags (all in `Config`, settable at a resume; off = the base learner)

- `fast_gather`: the content-stats gathers (and the event options' per-token gather) by
  `F.embedding`.
- `epochs=1` from the start; `epochs` and `e1_at` are now in `RESUME_SETTABLE`.
- `fixed_probe=false` (needs `kl_source="batch"`): skips the fixed probe's two passes per update;
  `kl` logs NaN. The batch probe still drives the lr factor and the early stop.
- `static_head`: the head's (row, action) pairs by `torch.nonzero_static` with the count from
  the host masks (`encode.legal_counts`), the run head over every pair and picked by
  `torch.where`; the collect's `pack` check is one sync at the collect's end; the batch probe and
  the practice bootstrap pass `y_rows`. No sync left in a forward except the collect's bucket
  widths (one per step) and the sampled actions.
- `pinned_copies`: the collect's host→device copies (obs, masks, priv, decision types, reward,
  done) from pinned buffers, non_blocking. A pageable copy waits for the GPU queue.
- `compile_encode`: `torch.compile(encode, dynamic=False)` for the collect's `envs`-row batches
  only (other sizes stay eager, so nothing recompiles); ~70 s compile at the start.
- `compile_net` (needs `compile` and `static_head`): `forward` and `policy` compiled whole,
  dynamic shapes, `cache_size_limit` 64. 30 recompiles in the first 8 iterations (0/1
  specialisation of the map-row count, the int32-index size guard), none after.
- Not done: `cstats.rows()` once per update. It is an 8,192×C×256 matmul per micro, under 0.1
  ms, invisible in the profile; caching it across micros would need retained graphs for no
  measurable gain. Fewer collect buckets and envs 4096 × steps 32 were tried: 4096×32 measured
  12,517 vs 12,506 (no gain), so the lane keeps 2048×64 and 3 buckets.

## 4. Same maths

- **All flags off = main, bit for bit:** a tiny CPU learner (full view, card stats, run head,
  two-chain, per-type norm, adaptive practice, behaviour) trained 3 iterations with main
  (a154862, a scratch worktree) and with this branch: identical weights, rewards and KLs.
- `fixed_probe=false` trains to the same bits as `true` (test).
- `static_head`: the same pairs and combat logits bit for bit; run-row logits within 6e-8 (the
  run head's matmul is now over more rows); gradients to float noise (test).
- `fast_gather`: the same forward bit for bit, gradients to float noise (test).
- GPU, one trained net, 2 micros, PPO loss gradients against eager base (relative L2 / max
  element): eager rerun 5e-10; `fast_gather` 7.4e-5; + `static_head` 7.4e-5 / 1.6e-4; the
  base lane's own block compile 2.7e-3 / 5.2e-3; `compile_net` with all 3.0e-3 / 5.2e-3. The
  compiled version is at the noise level the lane's compile already has (bf16 fusion order).
- `compile_encode` matches eager: integers exact, floats within 6e-8 (test, CUDA).
- Tests: `python/tests/test_rln44.py` (11); the whole Python suite passes (179).

## 5. Learning curve (15 min by wall clock, from scratch)

| minutes | base: decisions, floor | all flags: decisions, floor |
|---:|---|---|
| 3 | 0.79M, 3.15 | 1.31M, 4.50 |
| 6 | 1.57M, 6.53 | 4.06M, 12.00 |
| 9 | 2.36M, 9.43 | 6.68M, 14.47 |
| 12 | 3.15M, 11.27 | 9.31M, 15.00 |
| 14.5 | 3.67M, 12.36 | 11.40M, 16.32 |

Floor = mean of the last 5 iterations. The all-flags run is ahead at every minute. Per decision
it is slightly behind (11.7 vs 12.4 at 3.67M), as expected of E=1 vs E=2; the brief's measure is
per minute.

## 6. Net size and hp_coef

All flags, iterations 9–14:

| net | collect | update | total | GPU mem |
|---|---:|---:|---:|---:|
| 6×256 | 45,364 | 24,041 | 15,614 | 8.1 GB |
| 4×256 | 51,751 | 30,414 | 19,093 (+22%) | 7.0 GB |
| 4×192 | 57,366 | 37,541 | 22,690 (+45%) | 6.5 GB |

**Recommendation: 6×256.** It is AlphaDefect's known-sufficient size and now 3.6× the old
speed; 4×256 buys 22% for unknown capacity loss. A smaller net is a separate experiment, judged
on fight panels.

`practice.hp_coef` (deliverable 7): practice rows get c·(φ(after) − φ(before)), φ = HP/max HP
(live combat HP in a fight, the header's otherwise, 0 at a death). Per practice episode this sums
to −c·HP lost/max HP. Normal runs are unchanged, and 0 computes nothing (tested bit-identical).
A test checks the per-row term and that each finished episode telescopes. **Proposed c = 0.05**,
the low end of the report's 0.05–0.1. A fight that costs 30% HP then costs 0.015, a third of a
boss kill's 0.05. A practice episode already bootstraps into the run value, which prices some of
the HP loss, so the low end avoids counting it twice. The 15-minute result is below (§8).

## 7. NECRO5 (not started; for the review)

Lane `target/rl-checkpoints/NECRO5`, unit `necro5-e2e`, from scratch: no `prior`, no
`init_from`. It is NECRO3's args (§10 of the plan) with RLN37 adaptive practice, plus 6×256, the
run head, behaviour telemetry, E=1, the RLN44 flags and `hp_coef` 0.05. Until RLN44 merges, run it
**from the RLN44 worktree** (`tools/e2e_lane.sh` freezes that worktree's FFI build, c7fe421e39c7abbd,
and the unit runs that worktree's Python):

```sh
cd ~/spirebot-RLN44
N5=~/spirebot/target/rl-checkpoints/NECRO5
LANE_OUT=$N5 LANE_UNIT=necro5-e2e LANE_ARGS='--set obs_v2=true --set returns="two_chain"
 --set adv_norm="per_type" --set kl_source="batch" --set micro=1024 --set compile=true
 --set card_stats=true --set emb_std=0.4 --set v_warm=3 --set ent_flat=374000000
 --set obs_full=true --set obs_events=true --set discard_potion=true
 --set lr_factor_range=[0.3,2] --set ent_boost=true --set layers=6 --set run_head=true
 --set behaviour=true --set epochs=1 --set fast_gather=true --set fixed_probe=false
 --set static_head=true --set compile_encode=true --set pinned_copies=true --set compile_net=true
 --set practice={"enabled":true,"start":0,"max_uses":16,"min_active":2,"adaptive":true,"hp_coef":0.05}' \
 tools/e2e_lane.sh start
# watchdog: NECRO4's watch.json plus a cancels-per-run check (the rest -> Smith -> cancel loop)
mkdir -p $N5 && python3 - <<'EOF'
import json, pathlib
w = json.loads(pathlib.Path.home().joinpath("spirebot/target/rl-checkpoints/NECRO4/watch.json").read_text())
w["checks"]["cancels_per_run"] = ["<=", 2.0, 25000000]
pathlib.Path.home().joinpath("spirebot/target/rl-checkpoints/NECRO5/watch.json").write_text(json.dumps(w, indent=1))
EOF
systemd-run --user --unit necro5-watch --working-directory=$PWD/python \
  ~/spirebot/.venv/bin/python python/rln42_watch.py watch --out $N5
```

The watchdog reports and pauses; it never steers the policy. Smith thresholds may fail early in
pure PPO; until 100M those failures are data, not a stop. `cancels_per_run` in the 15-minute
runs: 0.004. NECRO4's rest→Smith→cancel loop produced dozens per run, hence a threshold of 2.
Expected: ~14–15k decisions/s and ≤ 9 GB GPU; the first ~8 iterations are slower while shapes
compile (~3 min).

## 8. hp_coef 15-minute check

All flags, 15 minutes from scratch, `hp_coef` 0 (§5) vs 0.05:

| minutes | hp 0: decisions, floor | hp 0.05: decisions, floor |
|---:|---|---|
| 3 | 1.31M, 4.50 | 1.18M, 4.03 |
| 6 | 4.06M, 12.00 | 3.93M, 11.17 |
| 9 | 6.68M, 14.47 | 6.55M, 13.65 |
| 12 | 9.31M, 15.00 | 9.04M, 15.41 |
| 14.5 | 11.40M, 16.32 | 11.01M, 15.53 |

| at the end | hp 0 | hp 0.05 |
|---|---:|---|
| practice elites survived | 66.6% of 817 | 65.2% of 1,051 |
| practice bosses survived (acts 1–2) | 19.6% of 929 | 11.0% of 1,015 |
| Smith at HP ≥ 70% | 0.006 | 0.056 |
| cancels per run | 0.004 | 0.000 |
| non-finite stats | none after iteration 1 | none |

It is finite and the same speed (13.7k decisions/s at the end). The floor curve is within one
seed's noise of hp 0. Bosses survived fell 19.6% → 11.0% in practice. That is one 15-minute run
each, and an untrained boss fight is mostly deaths, so it does not decide c either way. **The
HP term is not shown to help at 11M decisions.** The proposal stays c = 0.05, the brief's ask and
the low end of the report's range. If the review prefers to keep NECRO5 to one change against
NECRO3, use `hp_coef` 0: it is settable at a resume (`--set practice=...`), so the term can be
switched on later. Per-iteration `hp_reward` (the shaped sum) is now in the log; the 15-minute run
predates that line.
