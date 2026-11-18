# Python binding (`sts2`)

ctypes over the C ABI of `crates/sts2-ffi` (header `crates/sts2-ffi/include/sts2.h`), with zero-copy
numpy views on the engine-owned buffers. Needs numpy; PufferLib is optional (without it
`Sts2PufferEnv` uses a minimal stand-in for `pufferlib.PufferEnv` with the same buffers and
methods).

```sh
cargo build --profile ffi -p sts2-ffi                 # target/ffi/libsts2_ffi.so
python -m unittest discover -s python/tests           # smoke tests: 1,000 random legal steps each
python python/bench.py 64 20000                       # steps/s, the Rust driver's workload:
cargo run --profile ffi -p sts2-ffi --example ffi_bench -- 64 20000
python python/run_bench.py 64 5000                    # run env: steps/s and runs/s, likewise:
cargo run --profile ffi -p sts2-ffi --example ffi_run_bench -- 64 5000
```

- `sts2.Sts2Envs(n, starts=None, seed=0, threads=1)`: the batch. `obs`, `static_obs`, `masks`, `done`
  and `won` are views on the engine's buffers (read-only; they change in place at every `step`).
  `starts` is a start corpus (`cargo xtask sweep --policy greedy --emit-starts FILE`, or the bot's
  own fight starts from `sts2-diff collect-starts`) or the RL bench's W1-all starts. `final_hp`
  is the HP an episode ended on; `blindfold(seed)` resamples each episode's hidden part.
- `sts2.Sts2PufferEnv(num_envs, ...)`: a PufferLib native env. Without a `buf` its `observations` are
  the engine's buffer; with PufferLib's shared buffers (multiprocessing) they are copied in after
  each step. `action_masks` is the legal mask; an illegal action raises `Sts2Error`. With
  `with_static=True` each observation is the dynamic part followed by the static one.
- RLB11's combat value net: `sts2/combat_net.py` (features, net, export),
  `train_combat_puffer.py` (masked PPO on the GPU; needs PufferLib 3.0.0, see
  `docs/research/rlb11-value-net.md` for its install), `rlb11_starts.py` (shards),
  `rlb11_fixture.py` (the Rust parity fixture), `rlb11_paired.py` (paired whole runs).
- `sts2.Sts2RunEnvs(n, seeds=None, seed_range=None, master=0, cycle=False, character="Necrobinder",
  ascension=0, combat="agent")`: whole runs as episodes (stage 9). Seeds are a list, a `(start, end)` range or
  endless random seeds from `master`; `character=None` plays seed `s` as the `s % 5`th character. The action space
  is the 140 combat indices followed by the run-level ones (`layout.run_base`); decisions with one legal option are
  taken inside the env. `combat="greedy"` or `"random"` settles every fight inside the env, so the agent only makes
  run-level choices. `obs`, `masks`, `done`, `won`, `end` (how a run ended: `sts2.env.RUN_ENDS`) and `floor` (the
  final floor) are views on the engine's buffers; the observation's layout is `crates/sts2-rl/src/run_obs.rs`
  (offsets in `layout`). `sts2.Sts2RunPufferEnv(num_envs, **kwargs)` is its PufferLib env (+1 victory, -1 other
  ends; `infos` carry the final floors).
- End-to-end options (`docs/plans/necro-e2e.md` WP1; all off by default, `**sts2.E2E` turns on the plan's):
  `dedupe=True` keeps one mask index per action class (identical plays, potions, choice picks, deck-selection
  cards; `crates/sts2-rl/src/classes.rs`), `class_counts` (n × actions) holds each kept index's member count, and a
  decision with one class is taken inside the env. `turn_cap` / `decision_cap` end a run as `fight_capped` /
  `decision_capped` (losses, not deaths). `events=True` fills `events` (n × 4: floors entered, bosses killed, a
  bitmask of the acts whose boss died, won) for each step; `counters=True` adds saturation and unmapped counts to
  `stats()`. `sts2.action_table()` maps each index to its entity (kind, slot, sub-slot, phase).
- Resume (ABI 6): `restart(seeds, next_serial)` starts env `i` on a fresh run of `seeds[i]` and later runs from
  source serial `next_serial`; stepping `sts2.HOLD` leaves an env as it is. A learner replays each env's actions
  of its current run (holding envs with shorter histories) to land exactly where a checkpoint left it.
- The e2e learner (WP3/WP4): `sts2.e2e.encode` turns obs bytes into entity tokens on the GPU, `sts2.e2e.net` is
  the 4×256 net with the entity policy head and the privileged value module, `python/train_e2e.py` the
  synchronous PPO learner (its docstring has the reward, returns, PPO and checkpoint details), and
  `python/e2e_token_stats.py` the encoder's done-check.
- **Build checks.** Loading the library compares its build id (`sts2_build_id()`, a hash of the
  sources it was built from, `crates/sts2-ffi/build.rs`) with this checkout's (`sts2.source_hash()`)
  and its ABI version with the binding's, and raises `StaleBuild` on a mismatch: rebuild after every
  change to `sts2-core`, `sts2-rl` or `sts2-ffi`. `vocab_hash=` makes a batch refuse a library whose
  id vocabularies (`crates/sts2-rl/vocab/`) are not the ones a net was trained on.
- The library is found at `$STS2_FFI_LIB`, else the first of `target/{ffi,release,debug}/`. Build it
  with unwinding (any profile but `fast`): the ABI catches panics at every entry point.

New to RL? `learn/rl/index.html` is a short interactive course built around this env (open it in a browser; no build step).

## A10 baseline

```
cargo build --profile ffi -p sts2-ffi
python python/a10_baseline.py --seeds 200 --envs 64 --out target/a10-baseline
python -m unittest discover -s python/tests
```

Contract and the first numbers: `docs/specs/a10-training.md`. The ended episode's final
observation is `envs.final_obs` (the env resets before the step returns).

## Combat PPO

```
python3 -m venv .venv
.venv/bin/pip install numpy torch
.venv/bin/python python/train_combat_ppo.py --updates 60 --out target/combat-ppo.pt
.venv/bin/python python/eval_combat_policy.py --checkpoint target/combat-ppo.pt --seeds 50
```

The system Python has no torch and skips the trainer's test; use the venv for training and for
`python/tests` when you want the trainer test to run.
