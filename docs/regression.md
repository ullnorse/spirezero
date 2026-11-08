# Verification and regression

The gate is mechanical; the differential tests compare the engine with the real game. Numbers
below are the 2026-10 state; `STATUS.md` has the latest run.

## Gate

```
cargo xtask gate      # fmt --check, clippy -D warnings, test --workspace, wasm check
```

Run it in a worker's worktree before reporting, and again on `main` after the merge. A merge can
break a dependent crate that the branch's gate did not build; fix that in a follow-up commit on
`main`.

## The referee

The real game runs headless (`referee/`) and is only an oracle for differential tests, never a
dependency of the engine. Builds are immutable and content-addressed in `referee/builds/<hash>/`;
`referee/builds/current` is switched atomically, so runs that started keep their build.

- A worker that changes `referee/setup.sh`, `referee/tracer/` or the other referee sources runs
  `referee/setup.sh` in its worktree and sets `STS2_REFEREE=$PWD/referee/builds/current`.
- After merging such a change the orchestrator runs `referee/setup.sh` on `main`; `tools/status`
  prints the current hash.
- Traces are cached in `~/.cache/spirezero-referee-traces/<key>.jsonl.gz`, keyed on the build and
  every input that shapes a trace. A hit never starts the game; a miss plays it once. Deleting
  any part is safe; `tools/disk.sh --gc` drops traces unread for 30 days.

## Combat regression

```
tools/regress.sh 5
```

Every ported encounter, plus every loadout in `crates/sts2-diff/loadouts/` on the encounters it
names, at A0, A10 and one middle ascension chosen by a stable hash of the run name, against the
referee. Quiet, parallel, and already-passing runs are skipped by a cache keyed on the engine
files plus the entity modules a run reaches, so a content-only merge re-runs only the affected
closure. `REGRESS_NO_CACHE=1` forces everything; forced jobs still replay the engine and compare
but use stored referee traces, so only unseen action lists start the game.

`tools/diff.sh` plays seeds in one `gen-batch` process, the referee in one session, and compares
in one `compare-batch` process. The `regress` cargo profile is release optimisation with
incremental builds, debug assertions and overflow checks on (`[profile.regress]` in `Cargo.toml`),
so `REPLAY_CHECK` still runs; it is about 4x faster than debug per seed and rebuilds after an
engine edit in about 5 s.

Each checkout builds its own `target/`; never set a shared `CARGO_TARGET_DIR` (concurrent builds
of different branches clobber each other).

## Whole-run regression

```
tools/regress_runs.sh 8
```

The loadouts in `crates/sts2-diff/run_loadouts/` play a whole run from Neow: relics, cards and
gold are handed over through the referee's `run_loadout` (`RUN_LOADOUT=<file>
tools/diff_run.sh N` plays one). A new run-level relic, card or reward effect ships with one.
The act-jump loadouts in `crates/sts2-diff/act_loadouts/` start in Hive or Glory.

## The differential tools

| Tool | Compares |
|---|---|
| `tools/diff.sh N [ENCOUNTER]` | Combat seeds (the first divergent step and the entity). |
| `tools/diff_run.sh N` | Whole runs with a policy (`random`, `greedy`, `shopper`), optionally `CHARACTER`, `ACT1`, `ASC`, `RUN_LOADOUT`, `EVENT`, `REAL_SHOP=1`. |
| `tools/diff_events.sh` | Forced events on their own page (every option). |
| `tools/diff_ancients.sh` | Ancients in the ancient rooms. |
| `tools/diff_map.sh` | Map generation. |
| `tools/regress.sh`, `tools/regress_runs.sh` | The two regression suites above. |
| `tools/final_matrix.sh` | The sign-off matrix. |
| `tools/replay_history.py` | The player's and the community run histories (`history-replay.md`). |

`diff_run.sh` takes `SEED_PREFIX` and `SEED_FIRST` (`SEED_PREFIX=ROT261024S SEED_FIRST=193
tools/diff_run.sh 1`), and writes every divergence under `target/fixture-candidates/` with the
command that pins it. A pinned fixture goes to `crates/sts2-diff/run_fixtures/` only after a fix,
with the reason in the commit; `candidates/` holds failing ones on purpose.

## Final matrix (sign-off)

```
tools/final_matrix.sh --acts all 200
```

`--acts act1|hive|glory|both|all` (or `FINAL_MATRIX_ACTS`; default `act1`) is the sign-off for
every character at once:

- `act1`: the natural Overgrowth/Underdocks cells at A0/A10 for random, greedy and shopper, plus a
  date-rotating seed pass and the act's zero-hit check. Runs go on into Hive and Glory; a
  divergence anywhere in the run fails the cell.
- `hive`/`glory`: greedy and shopper at A0/A10 from an act-jump loadout, plus 200-seed rotating
  passes; `cover` passes with the act's whole card and relic pool; coverage, no-cover-divergence
  and the act's zero-hit check are hard failures.
- `both` is act1+hive, `all` is act1+hive+glory. `--hits-only` runs just the passes, a 2-seed
  regress and the zero-hit check.

Cells print `matching/total (reached..., reached/beat the boss...)`. The sign-off commands are
`--acts all 200`; the report goes to `history/signoff-<date>.md` (triage only: fix nothing in
the sign-off task, pin what is worth pinning).

## Coverage and hits

```
cargo xtask coverage                 # ported / in scope / missing per kind; exit 1 if any
cargo xtask coverage --allow-missing # same, exit 0
cargo xtask coverage --all           # the character-by-act matrix
tools/hits.sh                        # ported entities no generated run reached
tools/hits.sh --effects              # entities whose effect never ran (card played, potion used,
                                     # power changed); presence alone is not evidence
tools/hits_sweep.sh                  # every character and both acts at a few seeds
```

`hits` files written since 2026-10-27 also record the combat events (a played card, a used potion,
a changed power), so `--effects` separates "the entity appeared" from "the behaviour ran". The
run loadouts alone reach 497/558 cards, 64/64 potions and 231/242 powers; `tools/hits_sweep.sh`
reports the rest. Relic triggers are not attributed by the event log yet, so a relic remains
presence-only.

Multiplayer-only entities and the ids in `docs/coverage-exclusions.md` are not counted. A zero
hit is a failure unless it is in `docs/hits-allowlist.md` with a reason (`--check` enforces it).

## Verify `main`

`tools/verify_main.sh` runs the gate, regress and 100 random plus 100 greedy runs on a frozen
detached copy of `main` under `~/.cache/spirezero-verify`. Run it in the background after merges, never
against the live checkout.

## Disk

Every worktree has its own `target/` (3–5 GB debug, 6–8 GB with the regress profile). `tools/disk.sh`
shows checkouts and the shared caches; `--gc` removes build scratch older than three days, stale
profiles and traces unread for 30 days (safe while workers run). Workers keep scratch under
`target/` in their worktree, never `/tmp`, and delete large scratch before they finish. Details:
`disk.md`.
