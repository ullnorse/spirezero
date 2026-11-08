# Combat throughput profile (task PROF)

The experimental bit-exact fight kernel (PF6, `sts2-kernel`, `--engine kernel`): `docs/kernel-prototype.md`.

Current baselines (SpireZero vs spire-mentor), the `cargo xtask bench` workload and the ranked speed plan:
`docs/specs/perf-plan.md` (task PF1). This page is the earlier PROF profile.

## S4A′: copy hygiene and the snapshot (stage 4 merge A′)
`docs/specs/stage4-design.md` section 10.3. Base `1fa7ba8`; the lockstep pin bumped to it. No layout change, no
content edit.

**Snapshot.** The pre-step state was a fresh `CombatState` clone inside `Pending` on every snapshotted step (a full set
of allocations and their drop per Silent action). It is now `Combat::snapshot: Option<Box<CombatState>>`, allocated
by the first step that needs it, refilled with `clone_from`, and read back with `state.clone_from(&snapshot)` on a
decision; resolving a decision does not copy it again (it already holds the state the step restarts from). It is part
of the combat only while a decision is pending, so `clone`, `clone_from`, `==` and `Debug` skip it otherwise.
`Pending` holds the request, step, decisions and the event count, and `Combat` keeps a spare `Pending` box, so neither
a step nor a copy allocates one.

**Copy hygiene.** `HookIndex::listening_cards` is a `Vec` refilled with `copy_vec_from` (was a `SmallVec<[CardId; 4]>`
extended element by element). `CostLists` is flattened (section 1.3): per kind, every card's entries in one list ordered
by card and then by insertion, as two `Copy` lists (cards, entries), so a card's list is one slice and a copy is two
`copy_vec_from`s instead of a `Vec` clone per card. `Monster`, `Rng` and `DecisionRequest` clone field by field (no
derived `clone_from` left on the copy path; `Step` is `Copy`). `Combat::clone_from` matches the two pending sides
instead of calling `Option::clone_from` (which ran the box's drop glue on every copy: 13 per copy). `impl_clone_by_fields!`
has a `checked` form: debug and `regress` builds assert copy == source after `CombatState::clone_from`, and
`Combat::clone_from` does the same.

**No-allocation test** (`crates/sts2-core/tests/it/copy_alloc.rs`). A lookahead-shaped loop (every legal action applied
to one scratch combat refilled from the real one) over the 22 W1 starts of Ironclad, Silent and Necrobinder, three
fights each, played twice; the second pass's copies must not allocate. The counting allocator is the `allocation-counter`
dev-dependency (thread-local counts; a hand-written `GlobalAlloc` is `unsafe` code, which the workspace forbids). It
found what the per-copy profile could not: steps that end a combat threw away buffers the next copy then allocated again
(`CombatHistory::clear` was `*self = default()`, `remove_all_powers` and `take_powers_where` replaced the power list,
`clear_piles` took the piles), and a copy from a decision-pending combat reallocated `Pending` and `DecisionRequest`'s
lists. All of them now keep their buffers. Callgrind LA (100 seeds, 577,677 copies): 33,591 `reserve` calls inside
`CombatState::clone_from` at the base, none now; 879 `free`s remain, as at the base (878; not traced, and the
test counts allocations only).

**Instructions per action** (`perf stat -e instructions:u` over the whole process ÷ actions, `fast`, `--threads 1`,
`taskset -c 4`; W1 = `--mode fights`, LA = `--mode fights --seeds 1000 --lookahead`, runs `--runs 2000`, run env
`--runs 5000`):

| workload | base `1fa7ba8` | A′ | change | expected (10.3) |
|---|---:|---:|---:|---|
| W1 Ironclad | 4,858 | **4,843** | −15 | 0 ± 20 |
| W1 Silent | 8,095 | **5,958** | **−2,137 (−26%)** | −1,900 |
| W1 Regent | 4,801 | 4,784 | −17 | |
| W1 Necrobinder | 5,355 | 5,277 | −78 | |
| W1 Defect | 5,354 | 5,336 | −18 | |
| LA | 29,300 | **28,321** | −979 (−3.3%) | −1,300 (−4.5%) |
| LA `--fresh-copy` | 41,223 | 40,912 | −311 | |
| RL step (`--mode rl`, all characters) | 12,731 | **12,047** | −684 (−5.4%) | |
| RL step, Silent only | 14,749 | **12,394** | **−2,355** | −1,900 (prompting deck) |
| RL step, Ironclad only (resets) | 11,768 | 11,639 | −129 | |
| run env step (`accepts` copies) | 36,066 | **35,541** | −525 | −250 per copy |
| runs | 11,994 | 11,618 | −376 | |

Snapshot commit alone: W1 4,862, Silent 6,062, LA 29,188.

**The copy** (callgrind, LA 100 seeds, 577,677 copies, inclusive per copy): `Combat::clone_from` 1,466 → **1,276**
(−190), of which `CombatState::clone_from` 1,392 → 1,232 and its memcpys 16.1 calls, 519. LA falls short of 10.3's
estimate by about 60 per copy: the history lists were already refilled with `copy_vec_from` at the base (its 174 was
mostly the memcpy of `this_turn` plus the scalars, which stay), so the "174 → ~60" item did not exist. What is left of
the per-field logic (~710 per copy) is scalar and array moves (`clone.rs`: 141, the default `clone_from` of the `Copy`
fields), `Vec` bookkeeping in `copy_vec_from` (`vec`, `raw_vec`, `ptr`: ~320 across ~25 lists) and `Option`
matching (64): layout, merge B′'s territory.

**W1 callgrind diff** (220 × 10, 48,018 actions, per action): 5,324 → 5,309 (−15). `Option<Box<Pending>>` drop glue −13.2,
`Hooked::energy_cost` −11.6 against `CombatState::local_energy_cost` +13.8 (the flattened cost lists' lookup inlines
differently: +2 net), `CombatHistory::clone` −4.3, the `SmallVec` extend −2.6, `Combat::apply` −2.5,
`end_of_turn_cleanup` −2.5; `CombatState::clone` +7.8 and `Combat::clone` +2.1 (the per-fight start copy: the
listening cards and the four cost lists are `Vec`s now), `legal_actions_into` +6.7, `Combat::run` +4.7 (the snapshot
refill test and the spare box).

**Gates.** Every commit: `cargo xtask gate`, `cargo xtask lockstep` against `1fa7ba8` (0 differences), every digest
(fights `ad55be60f5bd8a71`, LA `85229a762f9361e1` both copy modes, runs `1febef7e3b528fe5`, 2,000 runs
`af6c19933b438172`, every character `125847415c9a423a`, 32k `bfb062b545664dda`, rl `372d441d8132f8de` with `--envs
1/64/256`, run-env `f174ae9e09ff5d14`, corpus `c7d24a331a7dc142` new format (sha256 `8e99c277…`, regenerated on the
base) and `77720b3f966bdeb9` old (`b1a30e7b…`)). The head also under `--pgo` (all identical);
`tools/replay_history.py --db --sample 300 --jobs 4` (3,000 runs, 0 engine mismatches, output and summary
byte-identical to the base's); `cargo test -p sts2-kernel --release` with `KERNEL_DIFF_FIGHTS=2000`;
`tools/regress.sh -j 4 5` (6,175 ok); `tools/regress_runs.sh -j 4 8` (499 ok).

## RC1: the core APIs the RL crates asked for (RL7, RL8 and RL9 core asks)
Base `ba3a26f`. Digests unchanged: fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, rl `372d441d8132f8de`,
run-env `f174ae9e09ff5d14`, corpus `77720b3f966bdeb9` on the old-format corpus (`sha256 b1a30e7b…`, which is the new
sweep's output with the two new fields removed, byte for byte). `cargo xtask lockstep` against `ba3a26f`: 0
differences.

**Core.** `Combat::energy_cost(card)` through the listener cache; `Hooked::intent_damage(monster, base)`, the fold
over a given base, and `Hooked::intents_into`, the intents on a caller-owned `IntentScratch` (refilled with
`clone_from`) along with each attack's base. A move's effects are compared under a second seed only when it gave
powers or cards and drew a random number. `MonsterKind::IDS` and `ordinal()` come from the build script (summons
included; the variants hold data, so there is an id list instead of `ALL`), `intent_is_dynamic` was not needed
(the oracle still passes, see RL7), `sts2_core::text` holds text forms for `RelicData` and `RunRngSet`,
`Combat::edit` sits behind the `testing` feature, and the run has `Run::legal_actions_into`, `RunCard::kind` (looked
up once in `RunCard::new`) and `Run::accepts`. `accepts` answers combat actions, gold, potion and card rewards,
picks, skips and plain proceeds from the run as it is, and anything else by applying it to a copy.

**RL.** `costs.rs` is gone; the intent cache keeps the bases and folds them again on a hit. The monster vocab
completeness test uses `MonsterKind::IDS`, so it covers summons. The leak test changes hidden state through
`Combat::edit`. The run observation reads `RunCard::kind`. The run env fills its list with `legal_actions_into` and
leaves actions `accepts` refuses out of the mask (before, the agent picked one, the env cleared its bit and the
agent chose again).

**Corpus records** now carry `relic_data` (each relic's `RelicData` and `IsMelted`) and `rng` (the run's streams
at combat start, kept by `Run::with_combat_start_streams`, which `--emit-starts` turns on). Older records still
load. Regenerated (`cargo xtask sweep --policy greedy --runs 20 --emit-starts FILE`): 846 starts, 1.52 MB, sha256
`8e99c277…`; `--mode rl --starts FILE --reps 50`: digest **`c7d24a331a7dc142`** (`--threads 1/3`), 42,300
episodes, 908,362 steps, `unmapped=4209`. The digest differs from `77720b3f966bdeb9` because the fights now start
from the run's own streams and relic fields instead of fresh ones.

**Run-env digest beyond the default workload.** With `--runs 5000` the digest changes (`402eebb6f11bc56f` →
`d1e7fd7e6c9ada43`) because the bench's random policy no longer draws a refused action and then draws again. Base
patched to drop refused actions from its mask up front (applying each to a copy) gives `d1e7fd7e6c9ada43` too. The
default 200 runs meet no refused action.

**Speed.** `taskset -c 4`, five ABAB rounds, median (all rounds within 1.5%); instructions are
`perf stat -e instructions:u` over the whole process. Base is `ba3a26f`.

| workload | base instr/step | RC1 instr/step | base steps/s | RC1 steps/s |
|---|---:|---:|---:|---:|
| fight env (`--mode rl`, 2,548,971 steps) | 24,374 | **12,870** | 455,504 | **798,521** |
| `--no-intents` | 11,110 | 8,731 | | |
| `--no-obs` (mask only) | 6,939 | 7,030 | | |
| `--no-mask` (the engine alone) | 6,193 | 6,300 | | |
| of which extraction | 18,181 | **6,570** | | |
| run env (`--mode run-env --runs 5000`, agent combat) | 40,822 | **36,018** | 238,348 | **257,949** |

Fight-env extraction is 18,762 before RL7 and 6,570 now: **2.86×** (the target was 2.8×). The run-env rows are
nearly the same workload, not quite (290,298 vs 290,340 steps; see above).

**Allocation in the run-env step.** Callgrind on the default run-env workload: the env layer no longer allocates in
steady state (only the vocab tables once and the observers at reset). What is left is the engine's own work (room
entry, combat set-up, shops, map and run creation at reset) and `Run::accepts`'s copy for the decisions it cannot
answer from the run: 3,984 copies over 200 runs, all accepted (map travel 1,469, card selections 863, ancient options
600, event options 312, shop purchases 471, other 269), about 33 allocations and 6,300 instructions for the copy alone
each, ~2,100 instructions per run-env step. Answering map travel without a copy would mean deciding ahead of time
whether the room's encounter, event, relics, deck and belt are all ported, behind the unknown-room roll. That is left
for the core lane.

## S1: hook dispatch by presence unions (stage 1 of the RL 2× plan)
`docs/specs/rl-2x-plan.md`, stage 1. Base `c15d570`. Digests unchanged: fights `ad55be60f5bd8a71`, runs
`1febef7e3b528fe5`, rl `372d441d8132f8de`, run-env `f174ae9e09ff5d14`, LA `85229a762f9361e1`. `cargo xtask gate`
passes; `cargo xtask lockstep` (pin `fc8ed3d`): 0 differences.

Instructions per action are `perf stat -e instructions:u` over the whole process divided by actions (W1 = default
`--mode fights`, LA = `--mode fights --seeds 1000 --lookahead`, RL = `--mode rl`). Wall time on `taskset -c 2`, ABAB,
7 rounds (LA, RL 5), median; the box was at load 2-4, so the rates are only indicative.

| | instr/action base | S1 | | rate base | S1 |
|---|---:|---:|---:|---:|---:|
| W1 LTO | 5,029 | 4,844 | −185 (−3.7%) | 110,000 fights/s | 114,000 |
| W1 PGO | 4,088 | 3,896 | −192 (−4.7%) | 148,300 fights/s | 150,500 |
| LA LTO | 29,528 | 29,287 | −0.8% | 19,860 fights/s | 20,260 |
| RL LTO | 24,373 | 20,695 | −15.1% | 19,630 fights/s | 20,670 |

**Short of the plan's −450.** Callgrind (line-table `fast` copy, W1 220 × 10) against the base: the old list's
upkeep is gone as estimated, about −390 per action (`bind` −83, the pile functions' in-place list maintenance
−150, `on_play_wrapper` −30, `cards_changed` −20, `rebuild` −10, `can_receive_powers` −11, `Combat::run` −11), but
a walk of the state is not free where filtering the cached list was cheap: W1 makes 1.4 passes per action with a
listener (half of them `fold_damage`), and each walks 2.7 creatures. What it took to beat the old cache at all:

- First cut (as left by the interrupted session): one out-of-line walk filling a `SmallVec`, 232 instructions
  per pass, +67 per action over the base. 110 of the 232 were the `allies.chain(enemies)` loop and a prologue
  into which LLVM hoisted every load of the items-and-cards part; the generated `MonsterKind::hooks` (a `match`
  over data variants) was a call per creature; `HookSet::contains` on a by-value set spilled it to the stack to
  index a word; and `Hooked::overriding` returned the 112-byte list by value (29 per action in `start_turn`).
- Fixes: two plain loops, the per-creature step inlined and the player's items and cards out of line;
  `Creature::monster_hooks`; one-word tests in place (`Hook::word_mask`, generated `hooks_ref`); then the sync
  passes (`each!`, `first_veto!`, the votes) visit the listeners during the walk, inlined at the site, so each
  union test of a known hook is one instruction and no list is built (5,063 → 4,938 → 4,844). The async
  `dispatch` builds its snapshot with the same walk (inlining `dispatch` itself into its callers was tried: 5,106,
  dropped).
- Left over: the visited listeners' calls (`kind.model()` and the dynamic call, as before), `count_card` +15
  (cards entering or leaving the piles, afflictions), `should_allow_hitting` +9, `memcpy` +9 (the `HookIndex`
  per-hook counts in each state copy). The binary is 3% larger (each `dispatch` instance carries its walk).

**After merging RC1** (main `b323b80`; lockstep pin moved there, 0 differences; digests unchanged). RC1's
`Combat::energy_cost`, `Hooked::intent_damage` and `Hooked::intents_into` now go through `Combat::hooked` /
`CombatState::hooked` (no listener list); `IntentScratch` no longer holds or clones one. Instructions per action,
`fast` (LTO) builds, `perf stat -e instructions:u`, main vs merged: W1 5,137 → 4,962 (−175, −3.4%); RL 12,871 →
12,834 (−0.3%). The RL gain above was mostly the list rebuild in each `CombatState` query, which RC1's cached list
had already removed from the fight env; S1 keeps RC1's numbers and adds the W1 saving.

## RL9: the run-level environment (stage 9 of the RL 2× plan)
`docs/specs/rl-2x-plan.md`, stage 9; `docs/reviews/spire-mentor.md`, ideas 1, 2 and 7. Base `ee29073`. No
`crates/sts2-core/src` edit. Digests unchanged: fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, rl
`372d441d8132f8de`, corpus `77720b3f966bdeb9` (corpus regenerated with `cargo xtask sweep --policy greedy --runs 20
--emit-starts`, sha256 `b1a30e7b…` as before, so moving the greedy combat policy changed nothing). `cargo xtask gate`
passes.

**The env (`crates/sts2-rl`).** `RunEnvs` (`run_envs.rs`) plays whole runs: reset is `Run::new` on the next seed of a
`SeedSource` (`seeds.rs`: a list or a range, once through or cycled, or endless random seeds from a master seed; seed
`s` is the game seed `RUN<s>`, the character fixed or `s % 5`). A step applies one index per env and then plays on
by itself to the next decision with more than one option: single options are taken automatically and counted
(`auto`), and an action `Run::apply` refuses (some listed actions hit unported content) changes nothing, has its
bit cleared and is counted (`refused`; no bit left ends the run as `Stuck`). Runs end as died, victory, unported
content, stuck or capped (60,000 engine actions); `done`, `won`, `end` and `floor` per env. Episodes go to envs in
env order after stepping, so results do not depend on the batch size or threads.
- *Action space* (`run_action.rs`): the combat space's 140 indices, then 215 run-level ones from `RUN_BASE` = 140:
  ancient, event and rest options (8 each), map travel by column (8), claim, skip (8 each), pick card (8 rewards × 5),
  card alternatives (8 × 2), use potion (6), deck-selection options (80), confirm, cancel, take relic, buy card,
  relic, potion (8, 4, 4), card removal, bundles (4), proceed: 355 indices in 6 mask words. The mask is filled from
  `Run::legal_actions`'s own list (`IndexMask`, the combat `Mask` made generic over its size); `check_run_mask`
  checks it against the list, in debug builds on every settle and in `tests/run_envs.rs` on every step of random
  runs (agent and greedy modes). Unmapped legal actions over 5,000 random runs: 70, all combat card choices with more
  than 10 options (the combat space's limit).
- *Observation* (`run_obs.rs`, 2,996 bytes): a header (character, ascension, act, phase, act id, floors, HP, max
  HP, gold, energy, belt slots, deck size, relic count, shop removals, the act's bosses, the current map point, the
  room's type and encounter or event), the deck × 80, relics × 32, the live belt × 6, the act's map × 96 points
  (column, row, type, visited/current/next flags, children), one section per decision kind (ancient options, event
  page as hashed option text keys, rewards with their cards, deck selection, bundles, rest site, treasure, shop
  stock with prices) and, during a fight, the combat observation and its static part. Content is in the append-only
  vocab ids, with three new files: `events.txt` (58), `ancients.txt` (8), `acts.txt` (4).
- *Leak test* (`tests/leak.rs`): at 1,000+ positions of 24 random runs (ancient, event, map, combat, rewards,
  rest and shop positions all covered), the run's hidden state taken from another seed's run (its and the player's
  RNG streams, the card, potion and `?` odds, both relic bags, every act's event and encounter lists and visit
  counts) plus a reseeded event `Rng` and the combat perturbation of RL8 leave the bytes unchanged. Adding one gold
  to the perturbation makes it fail.
- *Hybrid mode* (`resolve.rs`): `CombatMode::Agent` plays the fights through the combat indices; `Greedy` and
  `Random` settle every fight inside the env through the `Resolver` trait (`RunEnvs::set_resolver` plugs in
  another). `Greedy` is the sweep's greedy combat planner, moved from `sts2-diff` (which now calls it) unchanged.

**Interfaces.** C ABI version 2: `sts2_run_envs_new(n, &Sts2RunConfig, threads)`, `_step`, `_reset_all`, `_stats`
(actions, decisions, auto, resolved, refused, unmapped, ended, won, capped, stuck, reset), `_len`, `_{obs,mask,done,
won,end,floor}_ptr`, `_free`, `sts2_run_layout` (sizes and section offsets); the combat entry points are unchanged.
Python: `sts2.Sts2RunEnvs` and `sts2.Sts2RunPufferEnv` (zero-copy views; +1 victory, -1 other ends). Smoke test
(`python/tests/test_run_smoke.py`): 1,000 random legal steps on 8 envs, at least 10 runs, every
fight's mask in the combat block and every other decision's in the run block; plus the greedy mode and the errors.
All 8 Python tests pass (numpy 2.5.4, no PufferLib). Bench: `cargo xtask bench --mode run-env [--combat
agent|greedy|random] [--runs N] [--envs N]`, digest **`f174ae9e09ff5d14`** for the default (200 Necrobinder runs,
agent combat; the same with `--envs 5 --threads 4` and `--envs 200 --threads 2`); `--combat greedy`:
`f9c25321f9212fb4` (also `--envs 3 --threads 3`).

**Fuzz.** `fuzz/fuzz_targets/ffi_step.rs` sends one input in three to the run env: a seed list, range or master
seed, any character, ascension 0-10, agent, greedy or random combat, 1-4 envs, then random legal run-space actions
(1 in 64 batches illegal: `STS2_ERR_ILLEGAL`, no buffer changed), checking the return code against `done`, `won`
and `end`, the mask bits, a legal action in every env with a run in progress and `check_invariants` on every
fight in progress. **10 minutes clean**: `-max_total_time=600 -max_len=2048 -len_control=0`, 6,723 inputs (11/s;
whole runs under ASan are slower than RL8's fights), coverage 42,403 edges, 180,395 features, no crash, peak RSS
990 MB. (The run used the build before the map/vocab speed-up below, which does not change the observation.)

**Speed.** `taskset -c 6`, load 1.0, five interleaved rounds, median (ranges within 5%). A step is one agent decision
per env; runs are Necrobinder A0 with uniformly random legal play. Instructions are `perf stat -e instructions:u`
over the whole process, `Run::new` included.

| workload | runs/s | steps/s | instr/step | note |
|---|---:|---:|---:|---|
| Rust bench, agent plays fights, 5,000 runs, 64 envs | 4,567 | 265,209 | 36,679 | 58 decisions, 70 engine actions per run (12 automatic) |
| Rust bench, random resolver, 5,000 runs | 9,927 | 91,852 | 104,212 | 9 decisions and 56 resolved actions per run |
| Rust bench, greedy resolver, 500 runs | 358 | 9,453 | 1.14M | 26 decisions, 135 resolved actions per run; the planner simulates turns on copies |
| Rust through the ABI, agent, 64 envs × 10,000 steps | 4,305 | 247,803 | | `ffi_run_bench` |
| Python, same | 2,899 | 166,888 | | 67% of Rust |
| Rust through the ABI, agent, 256 envs × 3,000 steps | 4,174 | 242,629 | | |
| Python, same | 2,915 | 169,461 | | 70% of Rust |
| Rust through the ABI, greedy, 64 envs × 300 steps | 251 | 6,944 | | |
| Python, same | 236 | 6,514 | | 94% of Rust |

Python and Rust print identical decisions, actions, runs and observation digests (64 envs, agent: 639,999 decisions,
770,407 engine actions, 11,119 runs, `f5914bab22169e7b`). Random agents die early (floor 5 on average; greedy fights
reach floor 12.6), so these are short runs; the per-step cost is what transfers. The first profile showed
`run_obs::map` (linear `visited.contains` per point) at 13% and `SipHash` on the vocabulary's string ids at 7%; a
column bitmask per row and an FNV hasher took the agent step from 40,586 to 36,679 instructions (−9.6%). What is left
is spread out: the map section ~10%, the combat observation ~7%, card-id lookups ~4%, and the engine (new runs'
map generation, fight starts, play).

## PGO by default, `may_prompt` cache, smaller hot code (task PF15)
The levers docs/reviews/perf-architecture.md measured as safe. Base `20e71d1`. Timing runs are pinned to
`taskset -c 7` (its SMT sibling 15 was not reserved: two other workers and a referee pool ran throughout, load
2.5-10), so wall times are noisier than PF13/PF14's; `perf stat` cycles and instructions (user space) are
quoted beside them.

**Step 1: PGO as the shipped build.** `xtask/src/pgo.rs` is the one PGO pipeline: build `sts2-bench`
instrumented, play the committed training bench (`TRAINING`: the default mix on seeds from 1,000,000, one
thread), merge with rustup's `llvm-profdata`. `cargo xtask bench --pgo` uses it as before; `cargo xtask package`
now trains and builds the Linux TUI with it by default (`--no-pgo` to opt out); `cargo xtask pgo [--profile P]
[--target T]` makes a profile for any binary of this workspace and prints the build line. The plain LTO build
stays the reference for instruction counts.
- *A profile applies only to builds with the same cargo profile and the same `--target` choice.* LLVM matches
  functions by symbol name, and cargo's `-C metadata` (in every symbol) hashes the profile and the compile kind,
  though no longer `RUSTFLAGS`. Applied to a `portable` build, `fast`'s profile was silently ignored (W1
  300x50: 1,843 M instructions plain, 1,874 M "with" the profile, no warning); so was the same profile on a
  `fast` build with an explicit `--target x86_64-unknown-linux-gnu` (1,842 M). So the package trains a
  `portable`, `--target x86_64-unknown-linux-musl` bench (a static musl binary runs here). Windows and macOS
  binaries can't run here, so they stay plain LTO; Windows-gnu has no profiler runtime either, and zig's linker
  (the macOS builds) can't link an instrumented binary.
- *The package's training bench uses the system allocator* (`sts2-bench`'s new default feature `mimalloc`,
  off for it): the TUI has no mimalloc, and mimalloc's C build needs a musl C compiler this box doesn't have
  (`zig cc` rejects cc-rs's `--target=` spelling). The engine's counts don't depend on the allocator.
- *Effect on the package's build* (the bench built like the TUI: `portable`, musl, system allocator; W1):
  instructions 2,023 M → 1,744 M (−13.8%) at 300x50, fights/s 48,800 → 57,800 (+18%, 7 rounds ABAB, median;
  musl's `malloc` is why both are low).
- *Reproducible.* Two trainings give byte-identical profiles; in another target directory only the embedded
  binary id differs (`llvm-profdata show --all-functions --counts --ic-targets` identical). Before, the merge
  also took whatever profiles the instrumented build scripts and `sts2-macros` (run inside rustc) had left
  in the raw directory, which depended on what cargo rebuilt; they change the profile summary (hot/cold
  thresholds), not any engine function's counts. Now only the training run's file (`LLVM_PROFILE_FILE`) is
  merged. W1 PGO instructions rose 1.9% with the clean profile, but fights/s did not fall: −1.5% in a first
  session at load 7, +0.6% on a quiet box (load 0.9) and +0.6% in the final table, i.e. PGO's layout noise.
  The stray profiles are likely one reason PF13/PF14 saw PGO move on changes that did not touch fight code.
- *BOLT on top: dropped.* With Ubuntu's `bolt-22` (fetched with `apt download`, not installed), `cargo xtask
  bench --pgo --bolt` kept the digest and gave W1 cycles −2.7% (2,186 M → 2,126 M, IPC 2.55 → 2.62), fights/s
  +3.1% W1, +2.7% LA, +0.6% runs (7 rounds, load 10). Under the 3-5% bar for layout-only gains, it needs a
  tool rustup doesn't ship, and it can't apply to the Windows or macOS packages. `--bolt` stays a bench option.

**Step 2: `may_prompt` as maintained state.** `CombatState::prompting` counts the models in combat that may
prompt, exactly as the scan counts them (each card's model, affliction and enchantment; potions; relics;
each creature's powers and monster). It changes only through `push_card`, `set_affliction`,
`set_enchantment`, `set_potion`, `push_creature`, `push_power`, `take_power`, `take_powers_where` and
`remove_all_powers` on `CombatState` (cards never leave `cards`, creatures never leave `creatures`, a
monster's kind and the relics never change in combat); `Combat::new` counts once at the end. `may_prompt()`
reads it; debug and `regress` builds compare it with the scan on every call (the oracle: the regress runs and
the sweep passed with it), and unit tests, which edit the state directly, scan. The count is derived data,
so it is left out of equality and hashing (`PromptCount`); otherwise a test state built by hand would differ
from the same state reached by play. Instructions per action (callgrind, `fast`): W1 5,556 → 5,368 (−3.4%),
LA 32,768 → 31,567 (−3.7%), runs 12,519 → 12,370 (−1.2%), as the study's upper bound (−3.65%) predicted.
W1 `perf stat` medians, 7 rounds: LTO cycles −4.2% (IPC 2.10 → 2.12), PGO instructions −3.9% but cycles −0.2%.

**Step 3: smaller hot code. Dropped.** Counters per W1 action (`perf stat`, 1,100 x 50, 5-7 interleaved
rounds, medians):

| Build | cycles | instructions | L2 code fills | taken branches | op-cache misses |
|---|---:|---:|---:|---:|---:|
| step 2 | 2,117-2,207 | 5,353 | 190-202 | 542 | 419-424 |
| one non-generic `dispatch` (`&mut dyn FnMut`) | 2,130-2,183 | 5,452-5,457 | 198-202 | 549 | 421-423 |
| that plus `#[cold]`/out-of-line rare paths | 2,115-2,130 | 5,425-5,432 | 196-198 | 537 | 417-419 |
| generic loop, shared non-generic snapshot builder | 2,222 | 5,572 | 213 | 559 | 456 |
| the rare paths alone (generic `dispatch`) | 2,094 | 5,369 | 181 | 535 | 413 |

The non-generic `dispatch` did what it was meant to do to the binary (82 instantiations, 119,592 bytes, became
one of 2,714 bytes; text −97 KB) but not to the front end: L2 code fills and op-cache misses did not move, and
instructions rose 1.8% (callgrind: the shared body itself was neutral, but LLVM then left `after_card_moved`
out of line and grew `damage_into`). A W1 action touches only a few of the instantiations, and those were not
what overflowed the caches. The rare paths alone (`emit`'s push `#[cold]`, `Combat::run`'s `Unsupported` arm
`#[cold]`, `after_card_moved`'s pass out of line after an inlined "no listener" test) measured, on a quiet box
(load 0.9), W1 fights/s +0.5% (11 rounds), LA cycles −0.7%, runs cycles +0.3%: noise, so they were not kept
either.

**Step 4.1: `Hooked` by reference.** `Hooked` was the state, the listener slice and a copy of the 16-byte
union, built about 27 times per action; it is now the state and a `&ListenerCache` (two registers), with
`has(hook)` and `listeners()` reading through it, and `with_listeners` builds a temporary cache. That alone
was +6% instructions: `ListenerCache::refresh` stopped being inlined and every `hooked()` paid a call (430
instructions per action in it). `refresh` is now an inlined `fresh` test with the rebuild out of line.
Instructions per action: W1 5,368 → 5,189 (−3.3%), LA 31,567 → 30,698 (−2.8%), runs 12,370 → 12,217 (−1.2%).
W1 counters (5 rounds): LTO cycles 2,162 → 2,023 (−6.4%), L2 code fills 194 → 165, op-cache misses 423 →
382; PGO cycles 1,607 → 1,458 (−9.3%), IPC 2.76 → 2.87.

**Step 4.2: `legal_actions` without a static table.** Profile after 4.1 (W1, callgrind on a line-table
build): `legal_actions_into` 699 instructions per action (13%), of which `can_play_with` 69 per card at 3.9
cards per action, and 20 of those 69 the call's six register saves and restores. A per-kind table would not
help: `is_playable` (3 of 500 kinds override it) already compiles to a 2-instruction jump-table entry, and
`target_type` is about 15 per call. What costs is the general function on the common case. `can_play_with` is
now an inlined test (no listener overrides the keyword, energy-cost or `ShouldPlay` hooks, `CostLists` is
empty, and the card has no star cost: `HookSet::intersects` with a const set, two loads) followed by the
card's own fields (Unplayable, base energy cost, `is_playable`); everything else goes to `can_play_hooked`, the
old body, out of line. Debug builds compare the inline answer with it on every call. `#[inline]` and
`#[inline(always)]` on the whole old function were tried first: LLVM ignored the first, and the second cost
+1% (register pressure in the hand loop). Instructions per action: W1 5,189 → 5,080 (−2.1%), LA 30,698 →
30,563 (−0.4%, one `legal_actions` per real action), runs 12,217 → 12,131 (−0.7%). W1 counters: LTO cycles
−1.4%, taken branches 532 → 513; PGO cycles −0.8%. Not done: incremental legal actions (the set changes on
nearly every action, as the study said).

**Results.** Every kept step keeps every digest (fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, LA
`85229a762f9361e1` in both copy modes, 2,000 runs `af6c19933b438172`, every character `125847415c9a423a`,
32,000 runs on 16 threads `bfb062b545664dda`, LTO and `--pgo`) and passes `cargo xtask gate`, `tools/regress.sh
5` (6,175 ok), `tools/regress_runs.sh 8` (497 ok), the sweep (`--all --runs 1000 --strong`, table byte-identical
to the base's) and `tools/replay_history.py --db --sample 300 --jobs 4` (3,000 runs, output and summary
byte-identical to the base's; 0 engine mismatches). Steps 2, 4.1 and 4.2 were each verified on their own code.

Instructions per action (callgrind, `fast`; W1 2200 x 50, LA 200 seeds, runs 2,000):

| Step | W1 | LA | runs |
|---|---:|---:|---:|
| base `20e71d1` (and step 1: no engine change) | 5,556 | 32,768 | 12,519 |
| 2 `may_prompt` count | 5,368 | 31,567 | 12,370 |
| 4.1 `Hooked` by reference | 5,189 | 30,698 | 12,217 |
| 4.2 inline `CanPlay` | 5,080 (−8.6%) | 30,563 (−6.7%) | 12,131 (−3.1%) |

Wall time, fights/s (runs/s for runs), `taskset -c 7`, all nine builds interleaved, 7 rounds, median, load
9-10 (two other workers; the sibling core was busy, so absolute numbers are 4-5% below a quiet box's; W1 1,100
x 50, LA 150 seeds, runs 2,000):

| Build | W1 LTO | W1 PGO | LA LTO | LA PGO | runs LTO | runs PGO |
|---|---:|---:|---:|---:|---:|---:|
| base | 97,300 | 131,000 | 17,810 | 21,900 | 10,350 | 11,410 |
| 1 (PGO pipeline) | | 131,700 | | 22,260 | | 11,560 |
| 2 | 98,900 | 133,900 | 18,750 | 22,600 | 10,490 | 11,720 |
| 4.1 | 106,700 | 147,300 | 19,440 | 24,340 | 10,730 | 11,930 |
| 4.2 | 107,400 (+10.4%) | 148,400 (+13.3%) | 19,650 (+10.3%) | 24,380 (+11.3%) | 10,760 (+4.0%) | 12,000 (+5.2%) |

On a quiet box (load 0.9, same core, 7 rounds) base and step 2 measured W1 101,700 → 105,600 LTO and 136,500 →
140,400 PGO, LA 18,450 → 19,340 and 22,840 → 23,420, runs 10,710 → 10,830 and 11,750 → 12,020.

Counters per action, base → final (`perf stat`, user space, 3 rounds, medians):

| | W1 LTO | W1 PGO | LA LTO | LA PGO | runs LTO | runs PGO |
|---|---:|---:|---:|---:|---:|---:|
| cycles | 2,281 → 2,061 | 1,667 → 1,454 | 12,057 → 11,043 | 9,705 → 8,746 | 6,488 → 6,330 | 5,903 → 5,596 |
| instructions | 5,544 → 5,074 | 4,547 → 4,144 | 31,733 → 29,588 | 27,150 → 25,091 | 12,331 → 11,997 | 11,063 → 10,691 |
| IPC | 2.43 → 2.46 | 2.73 → 2.85 | 2.63 → 2.68 | 2.80 → 2.87 | 1.90 → 1.90 | 1.87 → 1.91 |
| L2 code fills | 201 → 171 | 127 → 101 | 904 → 744 | 575 → 458 | 370 → 344 | 278 → 254 |
| taken branches | 576 → 514 | 216 → 185 | 3,207 → 2,937 | 1,602 → 1,406 | 1,167 → 1,114 | 672 → 637 |
| op-cache misses | 443 → 392 | 326 → 277 | 2,194 → 1,873 | 1,656 → 1,349 | 825 → 781 | 678 → 639 |

What is left, by the study's list: lever 6 (`bind` re-deriving the model, about 1-1.5%), the stack buffers of
lever 8a (under 1%), and the `Copy` core state (lever 5 of the brief, the study's lever 7), which is the user's
decision.

## Stage 8 of the RL 2× plan: the C ABI, the Python binding and RL hardening (task RL8)
`docs/specs/rl-2x-plan.md`, stage 8 and section 7.4; `docs/reviews/spire-mentor.md`, ideas 2-5. Base `13d0a24`. No
`crates/sts2-core/src` edit. Digests unchanged: fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, rl
`372d441d8132f8de`, corpus `77720b3f966bdeb9` (the corpus regenerated with `cargo xtask sweep --policy greedy --runs
20 --emit-starts`, sha256 `b1a30e7b…` as in RL7). `cargo xtask gate` passes.

**`crates/sts2-ffi`** (new; `cdylib` + `rlib`, header `include/sts2.h`): the only crate that allows `unsafe`. It has
its own copy of the workspace lints with `unsafe_op_in_unsafe_fn = "deny"` (also `#![deny]` in `lib.rs`) and clippy's
`undocumented_unsafe_blocks` and `multiple_unsafe_ops_per_block` denied, so every block is one operation under a
`// SAFETY:` comment (16 in the library). Entry points: `sts2_envs_new(n, start_source_path, seed, threads)` (a
JSON-lines corpus, or null for the RL bench's 2,200 W1-all starts, `Order::Random` from `seed`),
`sts2_envs_new_from_corpus`, `sts2_envs_step(envs, actions, n)` (the number of ended episodes, or a negative
`STS2_ERR_*`), `sts2_envs_reset_all`, `sts2_envs_stats`, `sts2_envs_{len,obs_bytes}`, `sts2_envs_{obs,static,mask,
done,won}_ptr`, `sts2_envs_free`, `sts2_layout` (sizes and section offsets), `sts2_last_error` (per thread),
`sts2_abi_version`, `sts2_build_id`, `sts2_vocab_hash`. Every entry point runs under `catch_unwind`; a panic inside
a batch poisons it (`STS2_ERR_POISONED` afterwards). `Envs` allocates its buffers once and never resizes them, so
the pointers are stable from `new` to `free` (the ABI test checks the addresses over 3,000 steps and a reset). The
library is built in the new `ffi` profile (`fast`'s fat LTO and one codegen unit, but unwinding).

**Build checks.** `build.rs` hashes every file of `sts2-core`, `sts2-macros`, `sts2-rl` and `sts2-ffi` (`Cargo.toml`,
`build.rs`, `src/`, `vocab/`) plus the workspace `Cargo.toml` and `Cargo.lock` (SHA-256 over path, length and
contents in path order; first 16 hex digits) into `sts2_build_id()`. `python/sts2/ffi.py` computes the same hash
over its checkout (40 ms) and refuses a library whose id or ABI version differs (`StaleBuild`, with the rebuild
command); `vocab_hash=` makes a batch refuse other vocabularies.

**Python** (`python/sts2/`, README there): `Sts2Envs` with numpy views on the engine's buffers (zero copy: the smoke
test checks that the observation array's address is `sts2_envs_obs_ptr`), and `Sts2PufferEnv`, a PufferLib native
env (`pufferlib.PufferEnv` when importable, else a stand-in with the same buffers; reward +1 win, -1 any other end;
`action_masks`; with PufferLib's shared `buf` the observations are copied in). `python -m unittest discover -s
python/tests`: 1,000 steps of random legal actions on 16 envs plus the error, stale-build and layout checks; it
passes with PufferLib 3.0.0's `PufferEnv` (its sdist on `PYTHONPATH`, gymnasium 0.29.1, gym 0.23.1; PufferLib's
own build needs torch) and without it.

**Steps/s from Python next to Rust.** One workload for both: the default source, 64 or 256 envs on one thread,
env `i` playing the `SplitMix64(seed + i)`-th legal index each step (`python/sts2/policy.py`, vectorised in numpy;
`crates/sts2-ffi/examples/ffi_bench.rs`, the same loop in Rust over the same ABI). Both print the same actions,
episodes, wins and FNV of the final observation buffer (64 envs: 1,280,000 actions, 55,231 episodes, 18,823 wins,
`eea7c0ced30ed1dc`), so the views read exactly the engine's bytes. `taskset -c 6` on a quiet box (load 2.7),
interleaved, 5 runs each, median; instructions are `perf stat -e instructions:u`, the difference between 12,000
and 2,000 steps (start-up cancels), per action.

| 1 thread, 1.28M actions | instr/action | steps/s (median of 5) | vs Rust |
|---|---:|---:|---:|
| Rust through the ABI, 64 envs | 18,851 | 483,563 | |
| Python, 64 envs | 25,463 | 310,598 | 64% |
| Rust through the ABI, 256 envs | | 470,919 | |
| Python, 256 envs | | 349,438 | 74% |

Python's cost is per batch, not per env (about 420k instructions per `step` call at 64 envs: the numpy policy, the
ctypes call, the stats read), so larger batches amortise it. The Rust driver matches the RL bench's own step
(RL7: 18,774 instructions on the public API).

**Observation hardening (`crates/sts2-rl`).**
- *Leak test* (`tests/leak.rs`): at 4,000+ positions of random play over all 110 W1-all starts × 2 policies, the
  draw pile reversed and rotated and every RNG stream reseeded (the run's `RunRngSet` and each monster's `Rng`) leave
  the observation bytes unchanged (fresh observers, so no cache answers). `Observer::observe_state` observes a state
  without a `Combat` around it, which is how the test changes the state without a core API. It passes; dropping a
  card from the draw pile instead makes it fail.
- *Id vocabularies* (`vocab/{cards,relics,monsters,powers,potions,encounters}.txt`, `src/vocab.rs`): one id per
  line, the line number is the id (0 `<pad>`, 1 `<unk>`); 558 cards, 297 relics, 110 monsters, 242 powers, 64
  potions, 87 encounters. The observation writes these ids instead of enum discriminants (deck, relics, belt,
  encounter, combat cards, powers, monsters; the monster code was a 16-bit fold of the id). `vocab/pins.txt` pins
  each file's line count with the hash of that prefix; `tests/vocab.rs` fails if a pinned prefix changed (an id
  renumbered or removed: checked by swapping two lines), if a line is unpinned, or if the engine has a kind its file
  lacks; `STS2_VOCAB_APPEND=1` appends and pins. The monster file was seeded from the monster modules (MonsterKind
  has no `ALL`); its completeness test covers the monsters every encounter spawns over 8 seeds.
- *Fuzz target* (`fuzz/`, its own workspace; `cargo fuzz run ffi_step`): random game-shaped starts (character, an
  act and one of its ported encounters, ascension 0-10, 1-40 random ported cards with upgrades, up to 6 relics, a
  belt of up to 5 slots, HP, energy, gold, floor) into `sts2_envs_new_from_corpus`, 1-4 envs, then random legal
  actions (1 in 64 batches illegal: must return `STS2_ERR_ILLEGAL` and change no buffer) through `sts2_envs_step`,
  checking after every step the return code against `done`, `won ≤ done`, the mask bits, a legal action in every
  live env and `sts2_rl::check_invariants` (HP in `0..=max`, block, energy and stars ≥ 0, hand ≤ 10, pile
  conservation and each card's recorded pile). Built with ASan and debug assertions (the engine's own checks on),
  stable toolchain with `RUSTC_BOOTSTRAP=1`. The first run stopped after 7,902 inputs on `MadScience is invalid type
  None`: the generator's fault (a Mad Science deck card always has a Tinker Time type in its counter), fixed in the
  generator. **Then 10 minutes clean**: `-max_total_time=600 -max_len=2048 -len_control=0`, 61,448 inputs (102/s),
  coverage 39,366 edges, 204,340 features, no crash, no leak report, peak RSS 1,026 MB.

## Stage 7 of the RL 2× plan: the RL crate (task RL7)
`docs/specs/rl-2x-plan.md`, stage 7 and section 7. Base `6b3f9e5`. No `crates/sts2-core/src` edit. Instruction counts
are `perf stat -e instructions:u` over the whole process divided by steps; wall time on `taskset -c 4`, base and new
interleaved (ABAB), 5 runs each, median. Digests unchanged: fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, rl
`372d441d8132f8de` (the same at `--envs 1/7/256` and `--threads 3`, and with `--check-mask`, `unmapped=0`). `cargo
xtask gate` passes.

**`crates/sts2-rl`** (new): `action` (the 140-index space of section 7.2 and `Mask`, filled from
`legal_actions_into`'s list with each index's list position, so decoding reads the list back); `obs` (section 7.1,
RL0's layout: 972 dynamic bytes per step, 274 static bytes per reset); `costs` and `intents` (the caches below);
`envs` (`Envs`, section 7.4: `new(n, source, seed)`, `with_observation`, `reset_all`, `step`/`step_with(actions,
on_end)` with auto-reset, `obs`/`static_obs`/`masks`/`done`/`won`, `episode`, `snapshot`/`restore`, `threads(t)`:
episodes are handed out in env order after the envs stepped, so trajectories do not depend on `t`); `start`
(section 7.5: `StartRecord` as JSON lines, `StartSource` with `Sequential { reps }`, `Cycle` and `Random` orders).
The RL bench (`--mode rl`) now drives `Envs`; `--no-mask` keeps a plain loop (the engine alone), `--check-mask`
checks the mask against the list on every step, `--starts FILE` plays a corpus. The fixed space maps a target that
is the player itself (self-targeted potions) to target slot 0; what it still cannot map (a choice past 10 options, a
pet target) is counted as `unmapped`.

**Caches, and their oracle.** Both are per env and keyed by exact words (a value's derived `Hash` written out, not
hashed). The *listener state* is every creature's liveness, side and powers (every `Power` field plus its
`power_data`), the relics and the belt, `in_progress` and `side`: everything today's cost and damage listeners (all
powers or relics) read besides the card or attack itself. A generation counter moves when it changes.
- *Modified cost* of a hand card: valid while the generation, the whole `Card` (its pile included) and its local cost
  modifiers are equal; unplayable and X costs are returned as is, as `energy_cost` does.
- *Intents* of an enemy: a move with no attack intent is keyed by its *token* (the creature is in combat, its whole
  `Monster` including the move machine and RNG, the ascension); an attacking move by the token plus the generation,
  the monster's HP, max HP and block, the player's HP and max HP, the number of combat cards and the encounter data
  (Undying Sigil reads the dealer's HP). The spec's design caches only the base and folds the modifiers live; no
  public fold exists, so the fold is cached with its inputs in the key.
- Debug builds and the `regress` profile recompute every hit on the slow path (`slow_energy_cost`,
  `scratch_intents`: the swap points) and assert equality. Checked over the full RL bench (`cargo test --profile
  regress -p sts2-rl -- --ignored`, 2,548,971 steps, which also asserts the digest and the mask on every step) and
  over a greedy corpus (`--starts`, 846 starts × 20 reps, 364,235 steps): no stale entry.

| RL step (2,200 × 50, 64 envs) | base instr/step | RL7 instr/step | base ns/step (steps/s) | RL7 ns/step (steps/s) |
|---|---:|---:|---:|---:|
| full: mask + observation + intents | **24,975** | **18,774** | 2,190 (456.5k) | 1,711 (584.6k) |
| `--no-intents` | 11,868 | 11,007 | | |
| `--no-obs` (mask only) | 6,578 | 6,956 | | |
| `--no-mask` (the engine alone) | 6,213 | 6,230 | | |
| of which extraction | 18,762 | **12,544 (−33%, 1.50×)** | | |

Wall time per run (steps/s, sorted), base: 314.4k, 325.3k, 456.5k, 457.3k, 472.0k (two runs disturbed by other
load); RL7: 425.5k, 584.3k, 584.6k, 586.2k, 589.8k. An earlier ABAB round on the same code less the empty-mask
guard: base median 467.5k, RL7 584.4k. The variants were measured on that build (the guard adds 3 instructions per
step).
`--no-obs` costs 380 more than RL0's loop: the `Envs` bookkeeping (batch validation, the per-step `done`/`won`
pass, copying each mask into the batch buffer).

**Why not 3× on the public API.** Random play ends a turn every ~3 steps, and every new move is a cache miss
(0.52 per step); each miss runs `CombatState::intents` (a full state clone, the move on the copy and a fold with a
fresh listener list: 7,900 instructions). A hand drawn after the listener state ticked misses the cost cache
(about 2 slow `energy_cost` calls per step, 700 each). Per step: intents 7,764 (0.85 misses), costs about 2,400,
listener state 530, the rest of the observation about 1,100, mask and `Envs` 726. Even a perfect key would leave
the new moves and new hands on the slow paths: about 7,000 per step, an estimated 2.7× on extraction.

**The core asks, prototyped (uncommitted, reverted):** `Combat::energy_cost(card)` through the combat's listener
cache; `Combat` fold of the damage hooks over a base (`intent_damage`'s second half, target the player, dealer the
monster, `ValueProp.Move`); and `CombatState` intents with each attack's *base* on a caller-owned scratch state
refilled by `clone_from`. With the base cached by the token alone and the fold run live on every observation, and
costs read without a cache: **12,915 instructions per step** (720-729k steps/s, digest unchanged), extraction
6,685: **2.81×**, and the listener state is no longer needed (−530, ~3.0×). The oracle passed on 11,000 W1-all
episodes and 4,230 corpus episodes in the `regress` profile, so no move in that content reads state at perform time
(`intent_is_dynamic` would be for such a move).

**Start corpus.** `cargo xtask sweep --policy greedy --runs 20 --emit-starts FILE` writes one record per Act 1 combat
the runs enter: 846 starts (every character, both Act 1s, floors 2-17), 607 KB, sha256 `b1a30e7b…`. A record has no
RNG streams (its seed string makes fresh ones) and no relic data (relics start with their initial data): neither has
a text form in the core yet. `--mode rl --starts FILE` (50 reps): digest **`77720b3f966bdeb9`** (`--threads 1/3`),
42,300 episodes, 910,459 steps, 410k steps/s, `unmapped=3296`.

## Stage 0 of the RL 2× plan: oracles and workloads (task RL0)
`docs/specs/rl-2x-plan.md`, stage 0. Base `36e5482`. Instruction counts are `perf stat -e instructions:u` over the
whole process divided by actions (load-independent); wall time on `taskset -c 4` (PF15 timed on 6/7 meanwhile),
interleaved rounds, median. The bench digests are unchanged: fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`,
LA `85229a762f9361e1` (both copy modes), 2,000 runs `af6c19933b438172`, every character `125847415c9a423a`. `cargo xtask
gate` passes; `tools/regress.sh 5` (no cache): 6,175 ok.

**sts2-core, additive only (nothing in `combat/`):** `src/digest.rs` (new), `src/capacity.rs` (new), two `pub mod`
lines in `src/lib.rs`.

### Canonical digest (`sts2_core::digest`)
`canonical_digest(&CombatState) -> u64` and `canonical_parts` (the same words in labelled parts: combat, rng,
player, each creature, each pile, each card, history). It hashes the game state, not its layout: creatures in id
order with powers in application order (each with its `power_data`), removed powers, the piles in listener order,
every card with its cost lists, the player, every run RNG stream (counter and state words), the history and the
combat's own fields; it leaves out derived caches (`power_kinds`, the living-primary-enemy count, `Card::pile`, the
listener cache) and the next-id counters, so a stage that drops a cache keeps the digest. Leaf values (a monster,
keywords, costs, content data, history) go through their derived `Hash`: a stage that changes how one hashes must
keep its words or bump the lockstep base in the same change (the harness reports it at step 0 otherwise).

### Lockstep oracle (`cargo xtask lockstep`, `crates/sts2-lockstep`)
The working tree's engine and the base (`sts2-core-prev`/`sts2-diff-prev`, git dependencies on
`file://$HOME/spirebot` (the private checkout) at a pinned `rev`; **bump both revs to the merge commit after each stage**) run in
one binary (`regress` profile, so the engines' own debug checks run too). One macro instantiates the harness's
engine wrapper for both crates; everything compared leaves an engine as text or a digest.
- **Fights** (default 50,000): every character × all 87 encounters × ascensions 0, 10 and a hashed middle one, then
  every `tools/regress.sh` loadout (772 files) on its encounters at the same ascensions: 7,219 specs, fight `i` plays
  spec `i % 7,219` on seed `LS{i / 7,219}`. A hash of `i` picks, independently: events recorded and compared (half),
  uniform or play-biased policy (half; the kernel test's), and a long fight (400 HP, some upgrades; a quarter).
- **Runs** (default 100): whole greedy runs with the sweep's `--strong` loadout, every character, A0/A10, both Act 1s,
  capped at 5,000 actions (a few strong runs loop for hundreds of rounds against the Waterfall Giant); the greedy
  policy chooses on the working tree's engine and the base applies the same index of its own list.
- **Every step**: status, pending decision, canonical digest, the events since the previous step, the legal actions
  in order, the result of the chosen action; runs also compare their screen and, outside combat, a digest of the
  whole run state. The first difference ends the fight; the report gives the fight, seed, step, both values and the
  digest parts that differ with their state. `--first i --fights 1 --runs 0 --trace` replays fight `i` step by step.
- **Main vs main** (base `06e3ef5`, core identical): `fights=50000 actions=1324701 won=37918 capped=5 refused=0
  differences=0 | runs=100 actions=93055 won=57 capped=1 differences=0`, 34 s on 8 jobs (the fights 6 s). The same
  against the committed pin `1484e14` (this branch's last core change; after the merge it is in `main`), 60 s while
  the regress shared the cores.
- **Injected change** (`turn.rs`, `setup_player_turn`: +1 energy when `round == 3`; reverted): caught in 47,325 of
  50,000 fights (the rest end before round 3) and all 100 runs. The first report is fight 0 (Ironclad
  `AEONGLASS_BOSS` A0 seed `LS0`) at step 4, part `player`, energy 4 against 3 on turn 3; `--trace` shows action 4 is
  the second `EndTurn`, so step 4 is the first state the change touches.
- Cost: the gate now also compiles the base's `sts2-core` and `sts2-diff` once (debug, cached afterwards).

### W1-all (`--character NAME|all`)
W1's seeds and encounters with each starter deck, HP and relic (`--character all`: one line per character; without
`--character` the fights stay W1 and the runs Necrobinder, so no existing command changes).

| W1-all | digest | instr/action | ns/action (one run, loaded box) |
|---|---|---:|---:|
| Ironclad (= W1) | `ad55be60f5bd8a71` | 5,536 | 457 |
| Silent | `7312f60889bb34b0` | 8,463 | 761 |
| Defect | `f4250d5c7d5ad917` | 5,945 | 487 |
| Regent | `117c8c41b2e56d46` | 5,458 | 454 |
| Necrobinder | `1bfeaef01d08023f` | 5,935 | 499 |

### RL bench mode (`cargo xtask bench --mode rl`)
`crates/sts2-bench/src/rl.rs`, the plan's definition (section 3): `--envs N` fights per thread (default 64) stepped
round-robin; seed `s` is W1-all's start of character `s % 5` on encounter `s % 22` (the start corpus is stage 7);
a finished fight is reset from its next episode with `clone_from` (timed). A step writes the legal-action bitset
(the 140-entry space of section 7.2), writes the observation (section 7.1: 972 bytes per step, plus 274 static
bytes per reset: player scalars, 8 creatures × 12 powers, 64 cards in id order with pile, modified cost and flags,
the hand, 8 intents, a pending choice), draws a uniform legal action from the bitset with `SplitMix64(seed, rep)`,
decodes and applies it. The digest is FNV over (seed, rep, won, actions, the final state's canonical digest) per
episode, summed: **`372d441d8132f8de`** at the default 2,200 seeds × 50 reps, the same at `--envs 1/7/64/256`,
`--threads 1/3` and every variant (no legal action falls outside the fixed space, `unmapped=0`, and the bitset's
index order is the legal list's order, so the variants play the same trajectories).

| RL step (2,200 × 50, 64 envs) | instr/step | share | ns/step (median of 5) |
|---|---:|---:|---:|
| full: mask + observation + intents | **25,503** | 100% | 2,181 (458k steps/s) |
| `--no-intents` | 12,347 | | 1,009 (991k steps/s) |
| `--no-obs` (mask only) | 7,022 | | |
| `--no-mask` (policy draws from the list: the engine alone) | 6,659 | 26% | |
| of which intents (`CombatState::intents`, a scratch copy and move per enemy) | 13,156 | 52% | |
| of which the observation | 5,325 | 21% | |
| of which the bitset | 363 | 1.4% | |

The final build (the decode table widened to `u16`, so a choice of hundreds of options cannot overflow it) measures
25,475 for the full step, same digest; the variants above were measured just before. Extraction is **74%** of an RL
step today. The observation's 5,325 is mostly the modified cost of the hand cards:
`CombatState::energy_cost` builds a listener list from scratch per call (about 700 instructions per card, 3,580 per
step); without it the observation is about 1,750, in line with the plan's prototype (~2,250 with `i32` slots). Stage 7
wants a cost accessor that uses `Combat`'s listener cache (a core-lane API). `--envs 1` and `256`: 26,004 and 25,461
instructions per step, 475k and 437k steps/s (64: 458k); same digest.

### Capacity survey (`cargo xtask sweep --capacities`, `STS2_CAPACITIES=DIR`, `sts2-diff capacities DIR`)
`sts2_core::capacity::HighWater` samples every combat state between steps: the high-water mark of each list a hot
state would hold, where it was first reached, and how many sampled states exceeded 8, 16, ... 256 (the spill rate of
a list of that capacity). `sweep --capacities` prints the table after the sweep's; with `STS2_CAPACITIES=DIR` any
`sts2-diff` process (the regress jobs, the history replay) leaves its marks in `DIR` and `sts2-diff capacities DIR`
merges them. Sampling between steps sees lists that only grow within a fight exactly (cards, creatures, removed
powers, this turn's history); a peak made and undone inside one step is not seen (the draw pile during a shuffle,
bounded by `draw_plus_discard`; the enemy turn's history). The history replay never plays fights (a `.run` file has
no combat log), so it contributes run-level marks only: deck, relics, potion slots, sampled per floor.

Sources (all on this branch; files under `target/`, not committed):
- **G**: `sts2-diff sweep --runs 200 --policy greedy --capacities`: 1,000 greedy runs at normal HP and gold, every
  character, 202,110 states (9 s on 8 threads).
- **R**: `REGRESS_NO_CACHE=1 STS2_CAPACITIES=... tools/regress.sh -j 6 5`: every encounter and loadout of the
  regress, 6,175 runs, **6,175 ok**, 705,573 states.
- **S**: `cargo xtask sweep --runs 1000 --strong --capacities`, the check list's sweep (random policy, 9,999 HP,
  5,000 gold): 5,000 runs, 2,912 victories (the table is the base's), 3,208,987 states.
- **SG**: the same with `--policy greedy --runs 200`: 1,000 runs, 701,707 states.
- **H**: `STS2_CAPACITIES=... tools/replay_history.py --db --sample 300 --jobs 4`: 3,000 community runs (91,570 of
  92,085 checks match, 0 engine mismatches), 91,014 floors; run-level lists only.

`--strong` runs are a stress test, not a distribution: with 9,999 HP a greedy or random player can loop against a
boss for hundreds of rounds (Waterfall Giant, Knowledge Demon, Axebots up to round 1,264), so their cards, piles,
removed powers, cost lists and power data grow without bound. Size from G and R; use S and SG as the spill test.

| list | G max | R max | S max | SG max | H max | suggested hot capacity | states over it: G / R / S (ppm) |
|---|---:|---:|---:|---:|---:|---:|---|
| cards in combat | 208 | 131 | 2,593 | 2,049 | | 128 | 568 / 11 / 5,695 |
| creatures (ever created) | 14 | 15 | 21 | 36 | | 16 | 0 / 0 / 233 |
| enemies | 6 | 6 | 6 | 6 | | 8 | 0 / 0 / 0 |
| allies (player, pets) | 2 | 3 | 3 | 2 | | 4 | 0 / 0 / 0 |
| powers on one creature | 9 | 10 | 20 | 20 | | 16 | 0 / 0 / 23 |
| powers, all creatures | 23 | 21 | 32 | 40 | | 32 (one region) | 0 / 0 / 0 |
| removed powers, one creature | 29 | 47 | 184 | 368 | | 32 | 0 / 124 / 2,704 |
| removed powers, all | 32 | 47 | 227 | 470 | | 64 | 0 / 0 / 1,674 |
| hand | 10 | 10 | 10 | 10 | | 10 | 0 / 0 / 0 |
| draw pile | 185 | 60 | 1,749 | 1,255 | | 64 | 445 / 0 / 12,059 |
| discard pile | 190 | 60 | 1,752 | 1,810 | | 64 | 806 / 0 / 5,412 |
| exhaust pile | 25 | 85 | 223 | 805 | | 32 | 0 / 912 / 3,779 |
| play pile | 3 | 3 | 9 | 9 | | 8 | 0 / 0 / 0 (SG 1) |
| draw + discard | 206 | 68 | 2,540 | 2,024 | | (a shuffle's bound) | |
| history entries this turn | 107 | 100 | 1,114 | 393 | | 128 | 0 / 0 / 88 |
| relics | 12 | 10 | 59 | 16 | 31 | 32 | 0 / 0 / 1,310 (H 0) |
| potion slots | 5 | 3 | 9 | 5 | 8 | 8 | 0 / 0 / 2,788 at 8 (H 0) |
| orbs / orb slots | 8 / 8 | 10 / 10 | 10 / 10 | 10 / 10 | | 16 | 0 |
| run deck | 43 | 26 | 219 | 76 | 185 | cold `Vec` | H: 6.7% of floors over 32, 0.1% over 64 |
| cards with cost lists | 30 | 24 | 120 | 50 | | cold | |
| one card's cost lists | 13 | 15 | 154 | 775 | | cold | |
| power data entries | 10 | 9 | 59 | 228 | | cold | |
| pending choice options | 29 | 27 | 816 | 1,102 | | cold | G: 1,840 ppm over 8 |

The plan's rule (survey × 2, rounded to a power of two) applied to the maxima gives 512 cards and 512-card piles,
because single long fights set them; the suggested column instead takes the capacity whose spill rate is at most
about 0.1% of normal states, which the spill design (section 2.1) is built for. At those capacities the hot card
part is 128 × ~16 B = 2 KB, the pile regions (10 + 64 + 64 + 32 + 8 entries × 2 B) 356 B, the powers one 32-entry
region × 24 B = 768 B, removed powers a 64-entry region: inside the plan's 4-8 KB. The observation (section 7.1)
should follow: 64 card slots truncate 0.19% (G) and 0.31% (R) of states, so 128; 8 creature slots can be one short
(3 allies + 6 enemies were seen), so 10; 10 choice options truncate up to 0.18% of states (G), so 32.

### Silent's allocation profile (stage 0 item c): the replay snapshot, not the attack path
Callgrind on a frame-pointer, line-table `fast` build, W1-all 220 seeds × 10 (whole process; `target/scratch/*.cg`,
not committed): Silent 9,352 and Ironclad 6,095 instructions per action, a gap of 3,257. In Silent's run
`mi_malloc_aligned` is 13.2% inclusive (1,233 instructions per action: the plan's "~1,150 of malloc"), and 77% of
it is called from `CombatState::clone` (915,743 allocations, 15.3 per clone). `CombatState::clone` is called by
`Combat::run` **59,985 times for 59,765 actions**: once per step, 2,790 instructions each inclusive, plus 534 per
action dropping it (`drop_glue::<CombatState>` from `Combat::run`). That is 3,320 per action: the whole Silent gap.

The cause is `Combat::run`'s replay snapshot (execution model C): before every step it clones the state when
`CombatState::may_prompt()` holds, i.e. whenever any card in combat could raise a choice. Silent's starter deck
holds Survivor (discard a card), so every step of every Silent fight snapshots the whole state, 15 heap lists, and
drops it at the end; Ironclad's starter can never prompt and takes no snapshot. `StrikeSilent::on_play` and
`Neutralize::on_play` are not among `mi_malloc_aligned`'s callers in this build, so the plan's attribution to the
attack path does not hold here. Any deck with a card whose model reports `may_prompt`, in any pile, pays the
snapshot on every action.

**Fix, measured as a scratch prototype (not committed: `combat/` is PF15's lane):** keep the previous step's
snapshot in the `Combat` and refill it with `clone_from` (PF12's buffer reuse) instead of a fresh `clone` and drop.
Digests unchanged (W1-all Silent `7312f60889bb34b0`, Ironclad `ad55be60f5bd8a71`, Necrobinder `1bfeaef01d08023f`).
Silent W1 **8,463 → 7,040 instructions per action (−16.8%)**, `mi_malloc_aligned` 13.2% → 2.5% of the run. As
prototyped it costs the non-prompting decks +2.3% (Ironclad 5,536 → 5,669, Necrobinder 5,935 → 6,068): the extra
`Option<Snapshot>` field and its move on every step; the core-lane version should touch the spare only under
`may_prompt` (or keep it in the `Pending` box). The rest of the gap is the snapshot's copy itself (about 1,500 per
action), which stage 4's `Hot` memcpy shrinks. A narrower trigger (snapshot only when the step's own models may
prompt) would remove it for most steps but changes what `may_prompt` means; not proposed here.

## Pile moves (task PF14)
W1, `LA`, `LA fresh` and runs as in PF13. Base `4fce890` (PF13 merged): W1 6,006, LA 34,766, LA fresh 47,750, runs
12,900 instructions per action (callgrind, plain `fast` build).

**Profile at the base.** The LTO build inlines the pile helpers into `draw`, `add_to_pile`, `move_to_pile` and
`add_cards_to_pile`, so the breakdown is from a profiling copy with `#[inline(never)]` on each helper (W1 6,417
instead of 6,006: the shares hold, the absolute numbers are a little high). Inclusive instructions per action:

| Callee | Calls/action | W1 | per call | LA | runs |
|---|---:|---:|---:|---:|---:|
| `move_to_pile_bottom` (the batch: flush and shuffle) | 0.46 | 260 | 560 | 1,579 | 298 |
| `Engine::cards_moved` (of it `card_listens` 121, `ListenerCache::cards_changed` 35) | 3.51 | 262 | 75 | 1,594 | 283 |
| `remove_from_pile` (of it the shift about 70, the search about 30, `pile_of` 21) | 3.04 | 260 | 85 | 2,260 | 370 |
| `after_card_moved` (`pile_of`, then `AfterCardChangedPiles` and `Late`) | 4.85 | 211 | 43 | 1,027 | 177 |
| `insert_into_pile` | 3.04 | 148 | 49 | 1,186 | 227 |
| `can_add_to_pile` (`hooked().is_ending()`, player alive) | 2.85 | 92 | 32 | 734 | 128 |
| `draw` itself (loop checks, `CardDrawn` history entry about 25) | 1.54 cards | 210 | | | |

By kind of work, W1: listener and hook bookkeeping (`cards_moved`, the after-move hook tests,
`can_add_to_pile`) about 565, copying (inserts, the remove shift, the batch's `retain`, `extend` and moved list)
about 500, searching (a card's place in its pile, the batch's checks) about 130, history about 25. The two
`AfterCardChangedPiles` passes visit nobody in W1 (no listener overrides them), and no starter card listens, so
most of the bookkeeping decides that there is nothing to do, once per card.

**Design.** No representation change first: the piles are short (hand at most 10, a W1 draw pile about 5 when
drawn from), and PF9 measured inline piles at +4-6% because every access paid for them. The cluster is mostly
per-card fixed cost, so the steps cut that:
1. Bookkeeping per batch, not per card. The after-move pass (`after_card_moved`) is skipped when no listener
   overrides `AfterCardChangedPiles` or its `Late` pass (a pass that visits nobody changes nothing, so the test
   holds for every later card of the batch too). `cards_moved` tests whether a card listens with one table
   read per kind (a `bool` per card kind, built from `HOOKS` at compile time) and only when the cache is fresh
   (a stale cache is rebuilt anyway). `draw` checks `is_over_or_ending` just before `add_to_pile`'s own check;
   when no shuffle ran in between, the state is the same, so only the player's death is left to test.
   Expected −3% W1.
2. The batch (`move_to_pile_bottom`): one pass that checks, records old piles and counts cards per source pile;
   a source pile that loses every card is cleared instead of filtered (the shuffle's discard and draw piles,
   a flush of the whole hand). Expected −1.5%.
3. One card's move: `remove_from_pile` and `insert_into_pile` fused, the shift done by copies instead of
   swaps, the pile matched once. Expected −1-2%.
4. If the draw pile's front removal is still visible: a draw pile with a head offset (index-stable front).

Parity risks: none of these changes what moves where or in which order; the risks are the order of hooks (the
after-move pass still runs per card, in order, whenever a listener overrides it), the listener list
(`cards_moved` still re-places every listening card; `ListenerCache::get`'s rebuild oracle checks it in debug
and `regress` builds), `LISTENER_ORDER` (unchanged: the cache is keyed by pile rank and place), the shuffle (it
builds its list before the move; the batch's result order is unchanged and checked against the per-card path
in a test) and `MoveLog` (monster moves, not touched).

**Results.** Every kept step keeps every digest (fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, LA
`85229a762f9361e1` in both copy modes, 2,000 runs `af6c19933b438172`, every character `125847415c9a423a`,
32,000 runs on 16 threads `bfb062b545664dda`, all LTO and `--pgo`) and passes `cargo xtask gate`,
`tools/regress.sh 5` (6,175 ok), `tools/regress_runs.sh 8` (497 ok), the sweep (`cargo xtask sweep --runs 1000
--strong`: 5,000 runs of every character, 2,912 victories, 4,137,135 actions; table byte-identical to the
base's) and `tools/replay_history.py --db --sample 300 --jobs 4` (3,000 runs; output, summary and details
byte-identical to the base's). Each step was verified on its own commit.

| Step | Change | W1 | LA | LA fresh | runs |
|---|---|---:|---:|---:|---:|
| base `4fce890` | | 6,006 | 34,766 | 47,750 | 12,900 |
| 3.1 | bookkeeping once per move | 5,846 | 34,064 | 47,047 | 12,771 |
| 3.2 | the batch in one checking pass; whole piles cleared | 5,786 | 33,806 | 46,785 | 12,727 |
| 3.3 | one card's move: shift by copies, helpers inlined, random place out of line | 5,694 | 33,322 | 46,308 | 12,649 |
| 3.4 | a draw moves the draw pile's top card directly | 5,556 | 32,768 | 45,754 | 12,520 |
| 3.5 | debug oracles only | 5,556 | | | |

Instructions per action. All in: W1 −7.5%, LA −5.7%, LA fresh −4.2%, runs −2.9%.

Wall time, fights/s (runs/s for runs), `taskset -c 6`, all ten builds interleaved, 9 rounds, median, load
0.9-1.0 (LA and LA fresh 300 seeds, runs 4,000):

| Build | W1 LTO | W1 PGO | LA LTO | LA PGO | LA fresh LTO | LA fresh PGO | runs LTO | runs PGO |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| base | 96,300 | 132,300 | 17,920 | 22,630 | 12,380 | 15,670 | 10,780 | 11,950 |
| 3.1 | 98,600 | 133,900 | 18,300 | 22,820 | 12,600 | 15,690 | 10,840 | 11,860 |
| 3.2 | 99,800 | 135,300 | 18,400 | 22,850 | 12,670 | 15,740 | 10,830 | 11,950 |
| 3.3 | 100,700 | 135,600 | 18,570 | 22,810 | 12,710 | 15,740 | 10,880 | 11,990 |
| 3.4 | 100,800 (+4.7%) | 135,500 (+2.4%) | 18,460 (+3.0%) | 23,040 (+1.8%) | 12,710 (+2.7%) | 15,790 (+0.8%) | 11,030 (+2.3%) | 11,930 (−0.1%) |

LTO mostly follows the instruction counts; 3.4's −2.4% W1 instructions showed as only +0.1% wall (LA
−0.6%, within a round's spread). PGO gains are all under 3% and so inside its layout noise; none is claimed.

- **3.1 (−2.7% W1).** `lacks_after_card_changed_piles` (both passes lack listeners) makes `after_card_moved` a
  no-op test and lets `add_cards_to_pile` skip its second loop. `CardKind::listens` is a `bool` table built in a
  const block from `hooks()` (test `listens_matches_hooks`, debug assert on every read); `cards_moved` tests
  `fresh` first. `draw` calls `add_allowed_to_pile` when no shuffle ran since its `is_over_or_ending` test, which
  is `can_add_to_pile` less the owner's death (debug builds evaluate `can_add_to_pile` and assert it). The three
  parts measured −1.7% (first two) and −1.0%.
- **3.2 (−1.0%).** `move_to_pile_bottom` checks and counts per source pile in one pass, fills `moved` while
  setting the new pile, and clears a source that loses all its cards (the shuffle's discard and draw piles, a
  whole-hand flush) instead of `retain`. `batch_pile_move_matches_one_by_one` now also moves whole piles and every
  card; debug builds check that a cleared pile's cards all move.
- **3.3 (−1.6%).** The swap loop in `remove_from_pile` became a copy loop (one load and store per card; LLVM
  keeps it a loop, no new `memmove` calls: −0.6%), `remove_from_pile` and `insert_into_pile` are `#[inline]`
  (−0.8%; their out-of-line prologues were about 15 instructions per call), and the `Random` position's RNG draw
  is a cold helper (−0.2%).
- **3.4 (−2.4%).** In that same no-shuffle case the drawn card is the draw pile's top and the hand has room, so
  `CombatState::draw_top_to_hand` moves it without `pile_of`, the search for its place, the full-hand redirect
  or the position match, then `cards_moved` and `after_card_moved(card, Some(Draw))` as before (the card had a
  pile: no `AfterCardEnteredCombat`). More than the work removed: `move_to_pile` left the draw loop, which LTO
  then laid out tighter.
- **Tried and dropped:** giving an emptied source pile's larger buffer to an empty destination in the batch (a
  shuffle reallocates the draw pile on 0.13 of 0.135 shuffles per action, 0.5% of W1): +0.1%, the discard pile
  then regrew instead.
- **Not taken:** an index-stable (head-offset) draw pile. After 3.4 the front shift is about 0.5% of W1, and every
  pile access would pay the offset (PF9's inline piles cost +4-6% the same way). Specialising the play path
  (hand to Play, Play to discard): it already skips the search for the pile; under 0.5%.
- **Monster moves as data (step 4): not taken.** At 3.4, `roll_move` is 99 instructions per action inclusive
  (the `MoveTable` walk; the tables are already data), `perform_move` 54 self (one `dyn` call via
  `MonsterKind::model`, 4) and `execute_enemy_turn` 94 self; the move bodies (849 inclusive) are the attacks,
  block and powers they apply, which data would still run. Data-driven moves would save the dispatch and part of
  the walk, about 0.3% of W1, under the 1% bar, for a rewrite of 113 monster files.
- **What is left on W1** (3.4, LTO with line tables, self): `draw` 245 (now holding the draw's move), the batch
  (`add_cards_to_pile`) 207, `move_to_pile` 112; the shuffle's own work (RNG 66, sort 48, list 59) about 200.

## W1 profile round (task PF13)
W1 as in PF9/PF11; `LA` is `--lookahead` (copies refilled with `clone_from`, PF12) and `LA fresh` is the new
`--lookahead --fresh-copy` (each copy a fresh `clone`, as before PF12, so it compares with PF11's LA). Base
`18d5a75` (PF12 plus step 0). Every kept step keeps every digest (fights `ad55be60f5bd8a71`, runs
`1febef7e3b528fe5`, LA `85229a762f9361e1` in both copy modes, 2,000 runs `af6c19933b438172`, 2,000 runs of every
character `125847415c9a423a`, 32,000 runs on 16 threads `bfb062b545664dda`, all also under `--pgo`) and passes
`cargo xtask gate`, `tools/regress.sh 5` (6,175 ok), `tools/regress_runs.sh 8` (497 ok), the strong sweep
(byte-identical to the base's: 1,000 runs, 615 victories, 864,613 actions) and `tools/replay_history.py --db
--sample 300 --jobs 4` (3,000 runs; output, summary and details byte-identical to the base's).

Method as in PF11/PF12: callgrind `Ir` of the whole process divided by actions, on the plain `fast` build (W1
2200 x 50; LA and LA fresh 200 seeds, digest `1e94c286fcacb08e`; runs 2,000). Profiles on a line-table build.
Wall time: `taskset -c 6`, every build interleaved, 9 rounds, median (LA 300 seeds, runs 4,000).

| Step | Change | W1 | LA | LA fresh | runs |
|---|---|---:|---:|---:|---:|
| base `18d5a75` | | 6,180 | 35,689 | 48,667 | 14,191 |
| 1 | a card's pool entry by one hash lookup | 6,180 | 35,689 | 48,667 | 13,291 |
| 2 | modifier lists only when read; shuffle on precomputed sort keys | 6,108 | 35,317 | 48,301 | 13,254 |
| 3 | map points and card-pool lists allocated once | 6,108 | 35,317 | 48,301 | 12,983 |
| 4 | a stale listener cache's union is `ALL`: `lacks` is one test | 6,006 | 34,766 | 47,750 | 12,900 |

Instructions per action. All in: W1 −2.8%, LA −2.6%, LA fresh −1.9%, runs −9.1%.

Wall time, fights/s (runs/s for runs), `taskset -c 6`, all ten builds interleaved, 9 rounds, median, load 0.9-1.0:

| Build | W1 LTO | W1 PGO | LA LTO | LA PGO | LA fresh LTO | LA fresh PGO | runs LTO | runs PGO |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| base | 94,100 | 128,600 | 17,320 | 21,850 | 12,100 | 14,800 | 10,060 | 11,190 |
| 1 | 94,900 | 126,500 | 17,240 | 21,680 | 12,140 | 14,850 | 10,590 | 11,390 |
| 2 | 95,700 | 127,000 | 17,700 | 21,720 | 12,200 | 15,100 | 10,620 | 11,570 |
| 3 | 95,700 | 126,700 | 17,660 | 21,610 | 12,210 | 15,050 | 10,710 | 11,710 |
| 4 | 96,600 (+2.6%) | 127,300 (−1.0%) | 17,860 (+3.1%) | 21,830 (0%) | 12,360 (+2.1%) | 15,020 (+1.5%) | 10,810 (+7.5%) | 11,730 (+4.9%) |

LTO follows the instruction counts. PGO is inside its layout noise: step 1 changes only run-layer code (W1
instructions identical) and moved W1 PGO by −1.6%, every step-1 round below every base round, so the profile
trained on the new run code laid the fight code out differently; steps 2-4 then gained +0.7% on that. No PGO
gain is claimed.

- **Step 0: `--fresh-copy`.** The lookahead bench's copy is either the scratch fight refilled with `clone_from`
  (the default, PF12) or a fresh `combat.clone()` per candidate. Both digests hold. At the base, fresh copies
  are 48,667 instructions per action (PF11's LA: 48,647) against 35,689.
- **Profile at the base** (W1, callgrind, line tables; exclusive and inclusive per function, inlined code
  counted in the function it was inlined into, so the shares are of the LTO layout):

  | Exclusive | % | Inclusive | % |
  |---|---:|---|---:|
  | `Engine::damage_into` | 6.9 | `Combat::apply` | 83.7 |
  | `Engine::on_play_wrapper` | 5.6 | `Engine::end_player_turn` (enemy turn, next `start_turn`) | 49.3 |
  | `Engine::add_cards_to_pile` | 5.5 | `Engine::start_turn` | 40.2 |
  | `can_play_with` | 5.3 | `Engine::on_play_wrapper` | 24.2 |
  | `Combat::run` | 4.7 | `Engine::attack` | 20.1 |
  | `CombatState::remove_from_pile` | 3.8 | `Engine::execute_enemy_turn` | 18.5 |
  | `legal_actions_into` (hand loop) | 3.7 | `Engine::damage_into` | 17.4 |
  | `Engine::end_player_turn` | 3.6 | `Engine::draw` | 16.0 |
  | `Engine::start_turn` | 3.5 | `monsters::perform_move` | 14.5 |
  | `Engine::move_to_pile` | 3.5 | `Combat::legal_actions_into` | 11.7 |
  | `Engine::draw` | 2.9 | `Engine::add_to_pile` | 9.0 |
  | `Engine::add_to_pile` | 2.8 | `Engine::shuffle` | 6.7 |
  | `legal_actions_into` | 2.5 | `Engine::add_cards_to_pile` | 6.1 |
  | `Hooked::fold_damage` | 2.4 | `can_play_with` | 5.6 |
  | `Engine::attack` | 2.4 | `Hooked::fold_damage` | 4.2 |

  Flat: no function above 7% self. The listener-cache tests (`ListenerCache::lacks`, `refresh`, `get`,
  `HookSet::contains`, inlined everywhere) were about 4.5% by source line; the pile moves (the five pile
  functions above) 18% self; allocation about 5% (vector regrowth after the per-fight clone 2.4%: the damage
  fold's modifier list 0.5%, powers 0.4%, piles 0.7%, history 0.4%, listener entries 0.1%). Runs at the base:
  map generation 22% inclusive, `Character::of_card` 6% inclusive (5% self) and `RunCard::card_type` 5%,
  `removable_cards` 4.7% (its sort key is `card_type`), vector regrowth 7%.
- **Step 1.** `RunCard::info` found a card's character with `Character::of_card` (a scan of every character
  pool with string compares) and then scanned that pool again; `card_type` is `removable_cards`' sort key, so
  each run's card-removal screen paid it per comparison. `Character::pool_card` looks the id up in one
  `IdTable` (the hash table behind every `Kind::from_id`, built on first use) of every pool card, in
  `Character::ALL` order; `of_card` and `info` use it. The test `pool_card_lookup_matches_a_scan` checks it
  against the scan for every pool card. Runs −6.3%; fights do not use it.
- **Step 2.** Two changes, each under 1% alone (−0.83% and −0.34% of W1), kept together:
  - The damage, HP-loss and block folds pushed every listener that changed the value into a `Vec` (an
    allocation on about half of the 1.35 million damage folds in W1) that only the matching
    `AfterModifying*` dispatch reads, and that dispatch visits nobody when no listener overrides it. The
    folds now push only when the cache's union has the after-hook (`collect`); nothing runs between a fold
    and its dispatch, so the list is the same whenever it is read. Tried first and dropped: the lists as a
    `SmallVec<[Listener; 4]>` (`Modifiers`): +0.2% W1, as `damage_into` and `on_play_wrapper` each grew by
    about 35 instructions per action moving and dropping the inline list, more than the allocations saved.
  - The shuffle's .NET introsort (`StableShuffle`) compared two cards by indexing both and reading their id
    rank and upgrade. `stable_shuffle_by_key` reads each card's key (`cards::sort_key`: rank << 8 |
    upgrade) once and sorts (key, card) pairs with the same introsort, so it sees the same comparison
    results and leaves the same order (`keyed_shuffle_matches_the_comparator_shuffle`,
    `sort_key_orders_as_compare`). The sort went from 112 to 48 instructions per action, less the key pass.
  - Tried and dropped: piles cloned with room for 12 cards even when empty (the discard pile regrows 3
    times per W1 fight): W1 −0.45%, but LA fresh +4.7% and runs +0.35% (an allocation per empty pile per
    copy). `rotate_left` instead of the swap loop in `remove_from_pile`: +2%.
- **Step 3.** The map builder's point list (a point is large: two inline neighbour rows) grew from 2 to 64
  points, about six reallocations per map; it now starts with room for every point a map can have.
  `CardPool::cards` collected filtered pools (about 86 cards) from empty, five reallocations per reward or
  shop card; it now allocates at the iterator's upper bound. Runs −2.0%. Tried and dropped (no gain):
  `Combat::legal_actions` with room for 16 actions (the allocation moves to a slower size class), and
  `RelicGrabBag::populate` presizing its rarity lists (the counting pass cost what the regrowth did).
- **Step 4.** Every hook point tests `ListenerCache::lacks` (`fresh && !hooks.contains(hook)`), dozens of
  times per action. A stale cache now holds `HookSet::ALL` as its union (`new` and `invalidate` set it; every
  in-place update and `get` already check `fresh` first), so `lacks` is the bit test alone; debug builds
  assert the invariant on every call. W1 −1.7%, LA −1.6%, runs −0.6%.
- **What is left on W1** (after step 4): still flat. The pile moves are the largest family (about 18% self:
  a draw is about 250 instructions per card in `add_to_pile`, `move_to_pile` and `remove_from_pile`, each
  piece small). Monster moves as data (perf-plan) would save at most part of `roll_move` (1.6% inclusive) and
  `execute_enemy_turn`'s dispatch. Narrow ids (#9) shrink copies, not W1 instructions. The per-fight
  start-state clone and its regrowth (about 5%) are the bench's own cost; a bench that refilled one fight with
  `clone_from` would not measure the same thing.

## Allocation-light fight copies (task PF12)
W1 and `LA` as in PF9/PF11. Base `9c10c57`. Every kept step keeps every digest (fights `ad55be60f5bd8a71`, runs
`1febef7e3b528fe5`, LA `85229a762f9361e1`, 2,000 runs `af6c19933b438172`, 2,000 runs of every character
`125847415c9a423a`, 32,000 runs on 16 threads `bfb062b545664dda`, all also under `--pgo`) and passes `cargo xtask
gate`, `tools/regress.sh 5` (6,175 ok), `tools/regress_runs.sh 8` (497 ok), the strong sweep (byte-identical to the
base's: 1,000 runs, 615 victories, 864,613 actions; PF11's 864,673 predates later content merges) and
`tools/replay_history.py --db --sample 300 --jobs 4` (3,000 runs; output, summary and details byte-identical to the
base's).

Method as in PF11: callgrind `Ir` of the whole process divided by actions (W1 2200 x 50; LA 200 seeds, digest
`1e94c286fcacb08e`); fights/s on `taskset -c 6`, base and both steps interleaved, 7 rounds, median, load 1.0-1.4.

| Step | Change | W1 instr./action | LA instr./action | W1 LTO | W1 PGO | LA LTO | LA PGO |
|---|---|---:|---:|---:|---:|---:|---:|
| base `9c10c57` | | 6,175 | 48,652 | 92,800 | 127,600 | 12,010 | 14,930 |
| 1 | `Combat::clone_from` reuses every buffer; the lookahead bench refills one scratch copy | 6,179 | 35,897 | 93,700 | 129,100 | 17,080 | 21,390 |
| 2 | that refill skips the `memcpy` call for empty `Copy` lists | 6,177 | 35,624 | 93,900 | 128,700 | 17,420 | 22,020 |

LA: −26.8% instructions, +45% (LTO) and +48% (PGO) fights/s. W1 does not use the new path; its ±1% is noise.

- **Profile at the base** (callgrind, LTO with line tables, LA 100 seeds). `Combat::clone` was 32.8% of LA
  inclusive at 5.3 copies per action. mimalloc's `malloc` was 14% (11.1 million calls, 8.3 million of them inside
  `CombatState::clone`: 14.2 blocks per copy), `free` about 5%, and `finish_grow` after the copy another 2.9% (1.6
  million regrowths: the copy's lists were cloned at their exact length).
- **Why not the inline arena (plan stage 4) first.** Every block is a `Vec` that the content code indexes and
  grows directly (`creatures`, each creature's `powers`, the piles, `cards`, the history), so an arena of ranges
  means rewriting every access, with `unsafe` forbidden (bounds checks per access) and the +4-6% W1 cost PF9 measured
  for inline piles. The profile pointed at something cheaper: a search copies into a fight it then throws away, so
  the next copy can reuse that fight's buffers. No block then needs to be allocated or freed at all, which is what
  the arena was for, and no access path changes.
- **Step 1: `clone_from`.** A derived `Clone` has no `clone_from` of its own (it clones afresh and drops the old
  value). `impl_clone_by_fields!` (`lib.rs`) writes `Clone` for `Combat`, `Pending`, `Snapshot`, `CombatState`,
  `Player`, `Creature`, `ListenerCache` and `RunRngSet`: `clone` exactly as `derive` would, `clone_from` through each
  field's own `clone_from`, so every `Vec` and `String` keeps its buffer (`Vec<Creature>` refills each creature in
  place, powers included). Its patterns name every field (no `..`), so a field added later fails to compile until
  it is listed. `Pile`, `CombatHistory`, `PowerDataTable` and `CostLists` got the same by hand. The bench's
  lookahead now keeps one scratch fight per seed and does `copy.clone_from(&combat)` before each candidate action,
  as a search would; the copy holds the same fight as before, so the LA digest is unchanged. The test
  `fuzz_combats::clone_from_matches_clone` (60 random fights of every character, events on in half) refills one
  scratch fight (still holding another fight or another shape) before every action, checks it equals the source, and
  plays a random action on it beside a fresh `clone`. After it: 0 mallocs per copy (from 14.2), and the copy is
  about 1,270 instructions (from about 2,750) and 18.7% of LA. A plain `clone` is unchanged.
- **Step 2: empty lists.** `Vec::clone_from` of `Copy` items is `clear` plus `extend_from_slice`, which calls
  `memcpy` even for 0 bytes; most of a fight's lists are empty (`removed_powers`, `exhaust`, `play`, `pets`, `orbs`,
  `escaped`, `cost_lists`, ...), and there were 15 `memcpy` calls per copy. The fields marked `: copy` in the macro,
  the piles and the history lists go through `copy_vec_from`, which skips the call for an empty source.
  Instructions −0.8% only, but wall +1.9% (LTO) and +2.0% (PGO) against step 1 in a separate ABAB session (LTO:
  every step-2 round faster than every step-1 round; PGO: all but one), and +2.0%/+2.9% in the final table, so it
  is kept on wall time (the skipped calls were short, mispredicted `memcpy` calls).
- **Where the copy stands.** Per copy after step 2 (callgrind, each type's `clone_from` kept out of line): `Player`
  about 300 instructions (five piles), the creatures about 260 (about 90 each), the history about 175, the run's RNG
  streams about 130 (512 bytes plus the seed string), the rest (cards at 160 bytes each, deck, creature and enemy
  lists) about 500. That is about 6,500 of LA's 35,600 instructions per action; the rest is the engine playing the
  candidate actions. An arena would turn the 15 calls into one `memcpy` but not the bytes: perhaps 4-8% more on LA,
  for the rewrite above.
- **Not taken, by the numbers:** skipping unchanged parts of the copy (the source does not change between one
  action's candidates, but knowing what the previous candidate changed needs dirty tracking on every write), and a
  smaller `RunRngSet` copy (`Rng` stays `Clone`-only on purpose, PF11; about 1% at best).

## Speed round 3 (task PF11)
W1 and `LA` as in PF9 below. Base `3f260ff`. Every kept step keeps every digest (fights `ad55be60f5bd8a71`, runs
`1febef7e3b528fe5`, LA `85229a762f9361e1`, 2,000 runs `af6c19933b438172`, 2,000 runs of every character
`125847415c9a423a`, 32,000 runs on 16 threads `bfb062b545664dda`, also under `--pgo`) and passes `cargo xtask gate`,
`tools/regress.sh 5` (6,175 ok), `tools/regress_runs.sh 8` (497 ok), the strong sweep (table identical to the
base's: 1,000 runs, 615 victories, 864,673 actions) and `tools/replay_history.py` over the user's history (168 runs;
output, summary and details byte-identical to the base's).

Method: `perf` is blocked here (`perf_event_paranoid=4`), so instruction counts come from callgrind (valgrind 3.25.1
built from source into `~/.local`): `Ir` of the whole process, setup included, divided by actions; W1 at 2200 x 50,
LA at 200 seeds (`--seeds 200 --lookahead`, digest `1e94c286fcacb08e`). Fights/s are wall time on `taskset -c 6`,
ABAB against the step's base, 7 runs each (LA 5), median.

| Step | Change | W1 instr./action | LA instr./action | W1 LTO fights/s | W1 PGO fights/s | LA LTO fights/s |
|---|---|---:|---:|---:|---:|---:|
| base `3f260ff` | | 6,531 | 52,071 | 88,700 | 118,100 | 10,890 |
| 1 | `is_ending` reads a count of living primary enemies | 6,219 | 50,524 | 92,900 | 123,400 | 11,270 |
| 2 | listener list updated in place on creature and card changes; batch pile moves fill their list in place | 6,127 | 49,924 | 93,400 (step 1: 91,900) | 129,500 (step 1: 122,900) | 11,210 (step 1: 11,180) |
| 3.1 | `MoveMachine` is `Copy` | 6,127 | 49,924 | | | |
| 3.2 | `Card` 240 → 160 bytes: decimals in it packed (`PackedDecimal`), `deck_version` a `u32` | 6,157 | 49,582 | 94,400 (3.1: 93,900) | 132,300 (3.1: 129,400) | 11,610 (3.1: 11,360) |
| 3.3 | `Power` 176 → 24 bytes: `PowerData` in a side table by power id | 6,172 | 49,518 | 93,400 (3.2: 94,000) | 133,000 (3.2: 132,800) | 11,730 (3.2: 11,590) |
| 4.1 | a cloned pile keeps room for 4 more cards (`Pile`) | 6,174 | 48,647 | 94,000 (3.3: 93,500) | 130,900 (3.3: 131,700) | 12,150 (3.3: 11,730) |

Final, same session, `taskset -c 6`, 7 interleaved rounds (LA PGO 5), median, load 0.8-1.3:

| | W1 LTO | W1 PGO | LA LTO | LA PGO | W1 instr./action | LA instr./action |
|---|---:|---:|---:|---:|---:|---:|
| base `3f260ff` | 88,600 | 118,300 | 10,990 | 12,990 | 6,531 | 52,071 |
| PF11 | 93,600 (+5.6%) | 130,800 (+10.6%) | 12,010 (+9.3%) | 14,930 (+14.9%) | 6,174 (−5.5%) | 48,647 (−6.6%) |

Per-step fights/s in the table above are each against the previous step's build in the same session. Across
sessions they drift by about 1%, so the rows do not chain exactly.

- **Step 1.** `CombatState::living_primary_enemies` counts the living primary enemies in `enemies`; `is_ending`
  tests it instead of scanning the enemies (about 14 times per action). It is recounted (a scan) only when an
  enemy's status can change: an enemy's HP reaching 0 (`lose_hp_internal`, `set_max_hp`) or rising from 0 (`heal`,
  the `SetCurrentHp` of `FurCoat`, `WaterfallGiant`, `DecimillipedeSegment`, `PunchConstruct`), Minion added or
  removed (`apply_power`, `remove_power`, death's power removal), and the enemy list changing (`push_enemy`,
  `remove_enemy`). A creature's side never changes. Debug and `regress` builds compare the count with the scan on
  every `is_ending`; unit tests, which edit the state between steps, recount before each step. Load 1.4 when timed.
- **Step 2.** Each step's fights/s were timed ABAB against the previous step's build (in brackets).
  - Listener list. A scratch counter on every invalidation of a fresh list showed 0.17 rebuilds per W1 action, all
    from creature and card changes: an enemy's death (removed from the room, then its powers removed: two rebuilds),
    `Afflict`, the player's tear-down at combat end, a summoned enemy. `ListenerCache` now updates in place for a
    creature leaving combat (death, `Escape`, a move's removal: its power and monster entries go), entering it
    (`add_created_enemy`, `add_pet`: its entries go before those of every later creature), the powers death removes,
    the tear-down (the player's powers and every card entry go), and card changes (`cards_moved`, a new
    affliction or enchantment, a cleared affliction: the card's entries leave and re-enter where the card now
    sits, by pile in `LISTENER_ORDER` and place in the pile). `Afflict` on a card that already has the affliction
    no longer touches the list. Death of the player (hooks deactivated) still rebuilds. The rebuild oracle in
    `ListenerCache::get` (debug and `regress` builds) checks every use, so regress and the sweep checked the
    order. W1 6,219 → 6,178.
  - SmallVec. Callgrind puts the `smallvec` code at 2.7% of W1, almost all the inline-or-heap test
    (`capacity > inline_capacity`) on each access and drop. With `unsafe` forbidden, an own inline list without a
    heap spill would pay the same test as a bounds check, and the lists (cards drawn, moved or flushed, creatures
    hit) have no bound the engine may rely on. So the lists were left as they are and their copies were cut instead:
    `libc memcpy` was 2.4% of W1, and the largest caller was `add_cards_to_pile` (four calls per call):
    `move_to_pile_bottom` returned its 136-byte `MovedCards` in an `Option`, which was moved twice more. It now
    fills the caller's list (6,178 → 6,132). Tried and reverted: copying `Creatures` snapshots, attack target
    lists and pile appends element by element instead of through `memcpy` (+0.4%: LTO inlined the rest worse).
    Kept as a cleanup: `damage_into` reads its results through one slice instead of indexing (−0.08%).
- **Step 3, stage 1** (`docs/specs/copy-state-plan.md`). `MoveMachine` is `Copy`. `Monster` is not: it holds the
  monster's `Rng`, and `Rng` stays `Clone`-only on purpose (a silent copy of a stream, advanced in place of the
  original, would desync the game without any error). Its derived `Clone` is already a plain copy of plain data, so
  instruction counts did not change (W1 6,127, LA 49,924); `Copy` only matters for the inline arena (plan stage 4).
- **Where LA's time goes** (callgrind, 100 seeds, after step 2). `Combat::clone` is 33% inclusive, about 2,750
  instructions per copy at about 5.3 copies per action. mimalloc's `malloc` is 13.6% and `free` 7.6% (about 17
  heap blocks per copy), `memcpy` 10.4%, and `CombatHistory::clone` alone 4.9%. Stages 2-3 shrink the bytes copied
  (cards 240 B, powers 176 B each), not the number of blocks.
- **Step 3, stage 2: compact cards.** The plan's side tables keyed by `CardId` do not fit `Card` as it is used:
  the run layer builds standalone `Card`s (`RunCard::card`, the card vars of `try_var`/`instance_var`) that read
  `data` and `stale_calculated` themselves, so moving those into `CombatState` would split the var API. The size had
  a simpler cause. A `Decimal` (an `i128` and a scale) is 32 bytes at 16-byte alignment: `CardData` was 80 bytes for
  `SovereignBlade`'s two decimals, `stale_calculated` 48, and the whole card aligned to 16. `PackedDecimal` (new,
  `decimal.rs`) stores a decimal in 16 bytes at 4-byte alignment in the C# layout (96-bit magnitude, sign and scale),
  converting exactly both ways (`packing_is_exact`); `SovereignBlade`, `Thrash` and `stale_calculated` store one and
  compute on `Decimal`. `deck_version` is stored as an `Option<u32>` behind `Card::deck_version()`. A card is now
  160 bytes at 4-byte alignment (`CardData` 36). W1 instructions rose 0.5% (LTO laid out `add_cards_to_pile`,
  the shuffle sort and `target_type` slightly worse) while W1 wall time did not drop; LA lost 0.7% of its
  instructions and gained 2.1% in speed (2.4 KB less to copy per 10 cards). Load 1.3-7.5 when timed. Not done: the
  last 30 bytes (the costs' `upgrade_cap: Option<i32>`, `clone_of`, `stale_calculated`) would need sentinels or a
  side table for a few percent more.
- **Step 3, stage 3: compact powers.** `PowerData` was 144 of a power's 176 bytes, for the few powers with per-play
  slot arrays (`Afterimage`'s 16 plays, eight slots for `Storm`, `Calamity`, `Oblivion` and others). It moved to
  `CombatState::power_data`, a `PowerDataTable` (entries by `PowerId`, sorted; the ids only grow, so new powers
  append). `CombatState::power_data(id)` reads it (`None` without an entry), `set_power_data` and `power_data_mut`
  write it; a removed instance keeps its entry, since a model still running reads it. `None` entries compare and
  hash as absent, so two states that reached the same data by different paths stay equal. The content changes are
  mechanical (`power(owner, id).data` → `*power_data(id)`, about 100 sites; the diff, TUI and kernel readers too).
  A power is now 24 bytes. W1 instructions +0.2% and LTO wall −0.7%, inside the noise (all seven rounds overlap;
  PGO flat); LA −0.1% instructions and +1.2% wall, every round above the base's. Kept for LA.
- **Step 4: profile after step 3** (callgrind, LTO with line tables; inclusive shares).
  - W1 (500 x 50) is flat. The largest self costs are `can_play_with` 2.5%, `damage_into` 2.3%, `libc memcpy`
    2.2% and `on_play_wrapper` 1.5%; mimalloc is about 3.5% in all, the start-state clone per fight about 2%,
    and no turn-flow function stands out.
  - LA (100 seeds) is the copy. `Combat::clone` is 32% inclusive, at 5.3 copies per action. Inside it are 14.2
    `malloc`s per copy at about 65 instructions each (`malloc` 13.9% of LA in all), 17 `free`s at about 24
    (5.1%), `memcpy` (8.9%) and `CombatHistory::clone` (5.1%). After the copy, `grow_one` on 4-byte vectors
    (1.05 million calls, 2.8%) came from `move_to_pile` and `on_play_wrapper`. The piles were cloned at their
    exact length, so the first card into the hand, discard or play pile reallocated.
  - Taken: **4.1, piles keep room when cloned.** `Player`'s five piles are a `Pile`, a `Vec<CardId>` newtype
    (it derefs to the `Vec`) whose `Clone` allocates room for 4 more cards, like the history list of PF9 step 1.
    An empty pile stays unallocated. LA −1.8% instructions and +3.5% wall; W1 flat. Load 1.5-7.4 when timed.
  - Tried and dropped: a hand-written `Clone` for `Creature` and `Player` with the same room in the power
    lists and piles. It added more than it saved (+0.7% LA instructions): the 256- and 280-byte structs built
    by hand were copied into place by `memcpy` calls (+83 million), against 68 million saved in reallocations.
    The `Pile` newtype keeps the derived clones.
  - Tried and dropped: `allies` and `enemies` inline (`Creatures`, a `SmallVec` of 8), to save two
    allocations per copy: LA −0.5% and W1 +0.5% instructions (every access pays SmallVec's inline test).
  - Tried and dropped: `hp_loss_hits`, `spawned_enemies`, `potions` and `unported_potions` inline
    (`SmallVec`). No change in LA (48,647 → 48,642) and W1 +0.2%. The potion lists are empty in these benches
    (no allocation to save), and `SmallVec::clone` copies element by element.
  - Tried and dropped: smaller history entries, 64 → 48 bytes. `HistoryEntry` kept its turn's round, side and
    turn number, which `this_turn` already holds once for the whole list, and `PowerReceived`'s unread `amount`
    (a `Decimal`) aligned the whole entry to 16; that amount was packed. Instructions: W1 −0.6%, LA −1.4%. Wall:
    LTO +0.2% and LA +0.7%, but PGO −5% on W1, reproduced on two cores. That was 1.3% more instructions and,
    judging by the gap, a store-forwarding stall in copying the 4-aligned entry. With
    `#[repr(align(16))]` the PGO loss was still −2.5%: the training profile inlined differently
    (`attack_players_hits` into `attack`, `tick_down_duration` into `modify_power_amount_by`; +65 instructions
    per action). Under 1% on LTO and LA, so dropped.

## sts2-core adopts the kernel's design (task PF9)
W1 as above. `LA` is the lookahead bench (`--mode fights --seeds 1000 --lookahead`: before each action, the fight is
copied and every legal action is applied to a copy; digest `85229a762f9361e1`). Every step keeps the digests (fights
`ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, LA `85229a762f9361e1`). At the end these pass:
- `cargo xtask gate`;
- `tools/regress.sh 5` (6,175/6,175) and `tools/regress_runs.sh 8` (497/497);
- the strong sweep: 1,000 runs, 615 victories, 864,673 actions, the same table as `main`;
- `tools/replay_history.py --db`: summary and per-run details are byte-identical to `main`'s. Its one engine
  mismatch is also on `main`.

Instructions per action are LTO, from `perf stat -e instructions:u`, pinned, setup included.

| Step | Change | W1 | LA |
|---|---|---:|---:|
| base `10299c9` | | 6,739 | 68,783 |
| 1 | history buckets clone with spare room (a search copy's next entry does not reallocate) | 6,724 | 65,053 |
| 2 | a per-creature power-kind set: `HasPower`, `power_amount` and `is_ending`'s Minion check test a bit (debug builds compare each test with the scan) | 6,643 | 64,883 |
| 3 | cards are `Copy`: the cost-modifier and temporary star-cost lists move to a sparse side table, `CombatState::cost_lists` | 6,580 | 57,036 |
| 4 | a monster's `StateLog` becomes a `MoveLog` (length, last 3, states used; every rule looks back at most 3), and `Power` is `Copy` | 6,542 | 54,442 |
| 5 | this turn's history is one list with per-tag counts | 6,503 | 50,204 |

Final measurements, same session, `taskset -c 5`, load 3-11:
- Fights/s are three interleaved rounds. The box was shared, so rounds moved by ±10%; the instruction and
  cycle counts are the steadier figures.
- All cores means 16 pinned full-length processes (summed), two rounds.

| | W1 fights/s, 1 core | W1, 16 procs | W1 instr./cycles per action | LA fights/s, 1 core | LA, 16 procs | LA instr./cycles per action |
|---|---:|---:|---:|---:|---:|---:|
| `main` LTO | 69-72k | 726-731k | 6,739 / 2,864 | 6.5-6.8k | | 68,783 / 28,757 |
| PF9 LTO | 67-72k | 771-773k | 6,504 / 2,646 | 9.4-10.0k | 91k | 50,205 / 20,896 |
| PF9 PGO | 85-104k | 1,041-1,046k | 5,211 / 1,947 | 10.5-11.1k | 108-110k | 43,615 / 17,077 |
| kernel LTO | 264-289k | 2,808-2,856k | 2,394 / 747 | 49-54k | 580k | 11,391 / 3,517 |
| kernel PGO | 211-240k | 2,240-2,264k | 2,992 / 906 | 44-47k | | 14,510 / 4,216 |
| spire-mentor `8e0e917` | 64-69k | 701-708k | 6,612 / 3,046 | (no lookahead mode) | | |

- On lookahead, sts2-core went from 6.0× to 4.4× the kernel's instructions. On W1, PGO uses 1.5× fewer cycles per
  action than spire-mentor; LTO alone uses 1.15× fewer.
- Under `--pgo` the kernel is slower than under LTO. The training mix is mostly sts2-core, so the profile lays the
  kernel's code out worse.
- `cargo xtask bench` (no taskset; load about 6): LTO 85.5k, PGO 106.7k fights/s.

Brief items not done, and why:
- **Inline piles (item 3): dropped.** One `SmallVec` per pile cost +4% W1 instructions (6,724 to 6,993), and LA gained
  1.7%. One five-range slab cost +6% W1 and gained nothing on LA. Every pile operation pays the inline/heap check.
- **Fixed-point math (item 1): not started.** `Decimal`'s integral fast paths (PF4, PF5) already put decimal work
  under 2% of W1. The kernel's `i128` proof covers only damage and block.
- **Presence masks for every hook family (item 2): not extended.** `Engine::lacks` already skips every hook nobody
  overrides (PF4), and the step 2 bit set covers the per-creature power checks. No hook-family pass shows in the
  profile.
- **Monster moves as data (item 4): not started.** `roll_move` and `execute_enemy_turn` together take about 3% of W1.
  The win would be structural (real codegen for the kernel's tables), not speed, so it belongs in its own task.
- **Cheap clone (item 5):** steps 1, 3, 4 and 5. A W1 clone 6 actions in still makes about 17 allocations (5.5 KB).
- **`Copy` state (item 6):** see the plan in `docs/specs/copy-state-plan.md`.

The W1 profile is flat now: no function's self time exceeds 7.4% (`on_play_wrapper`, `damage_into`, `draw`,
`can_play_with`).

## Speed round 2 (task PF5)
W1 as above. Every step keeps the digests (fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`); at the end
`cargo xtask gate`, `tools/regress.sh 5` (6,175/6,175), `tools/regress_runs.sh 8` (497/497) and the strong sweep
(1,000 runs, 615 victories, 864,673 actions, as on `main`) pass. Instructions per action from `perf stat -e
instructions:u`, 2200 x 50 pinned, setup included.

| Step | Change | LTO instructions/action |
|---|---|---:|
| base `dddc605` | | 9,375 |
| 1 | the no-listener check of the hot `Hooked` folds (`modify_hp_lost`, `modify_card_play_count`, `keywords`, power amounts, `modify_damage`, `should_play`) inline, the fold out of line | 8,868 |
| 2 (item 2) | `with_model!` (build.rs): static dispatch of card/power model queries (target, type, `IsPlayable`, costs, keywords) instead of `model()` + `dyn` | 8,698 |
| 3 | `record` and `CombatHistory::push` inlined: the entry is stored from registers (it was copied through the stack, a store-forwarding stall) | 8,417 |
| 4 | damage results pushed into the caller's list; creature lists copied as slices; the engine borrows the listener cache; history key, CardPlayStarted entry and attack targets without stack round trips | 8,162 |
| 5 | `may_prompt` and the other per-step model queries of every kind statically dispatched (no `LazyLock` tables) | 7,969 |
| 6 (item 6) | batch pile moves (Shuffle, flush) at once when they cannot call a hook (`docs/semantics/combat-core.md`, "Batch pile moves") | 7,750 |
| 7 | monster moves rolled without cloning the move machine and its log | 7,632 |
| 8 | `CanPlay` reads plain energy and star costs inline | 7,335 |
| 9 (item 3) | `Decimal`: `i64` normalization/truncation, small-operand products, integral `+ - * cmp` inline | 7,083 |
| 10 | fast paths for `modify_block`, `CheckWinCondition`, `CardPileCmd.Add`'s guard; turn-end cleanup in one pass; dispatch snapshot by reference; single-target hits without a list | 6,882 |
| 11 (item 4) | powers enter and leave the cached listener list in place (debug/regress builds check it against a rebuild on every use); `Afflict` uses the cached list | 6,760 |

Final, `taskset -c 9`, three interleaved rounds at load 3: base 57,200, PF5 LTO 75,700, PF5 PGO 98,100,
spire-mentor `8e0e917` 73,800 fights/s (LTO 1.03x, PGO 1.33x). Cycles per action: LTO 2,776, PGO 2,184,
spire-mentor 3,010 (best of 3); instructions per action LTO 6,760, PGO 5,679, spire-mentor 6,612. L1d misses
84M -> 61M, branch misses 46M -> 35M (base -> LTO, whole run).

Scaling (N pinned full-length processes on cpus 0..N-1, summed fights/s, load 3-7; cpus 8-15 are the SMT
siblings):

| N | 1 | 2 | 4 | 8 | 16 |
|---|---:|---:|---:|---:|---:|
| PF5 LTO | 79k | 155k | 285k | 500k | 721k |
| spire-mentor | 73k | 144k | 277k | 484k | 706k |

PGO on 16: 927k. `--threads N` scales the same (sum of per-thread rates 79k, 150k, 298k, 536k, 728k), so
there is no false sharing: threads share only read-only statics, and mimalloc's heaps are per thread.

Kept or dropped, by brief item:
- 1 (allocator): dropped. With the engine's allocations down to about one per action, glibc instead of mimalloc
  costs 2.4% instructions and about 1% cycles, so mimalloc stays the bench's allocator and the engine (a
  library) sets none; a per-fight arena would save less than that. snmalloc was not measured (its crate needs
  cmake, which this box lacks).
- 5 (layout): `Decimal` as sign plus 96-bit magnitude (16 bytes, `Card` 272 -> 192 bytes) cost +5% instructions
  (conversions on every operation), packed to 24 bytes +0.6%: dropped. Most remaining store-forwarding stalls
  (a struct built field by field and copied with wide loads) did not move cycles beyond noise once found by
  instruction-level sampling; only the ones above were kept. Narrow ids were not tried.
- 9 (audit): the replay check (`REPLAY_CHECK`) and the listener-cache oracle are `cfg!(debug_assertions)`; the
  remaining release `assert!`s are O(1) checks mirroring the game's exceptions. Nothing to change.
- 10 (scaling): near-linear per physical core, about 9x on 16 threads; nothing to fix.
- Tried and dropped: `#[inline]` hints alone (LTO ignored them), `-inline-threshold` 500/1000 (within 1%),
  precomputed sort keys for Shuffle (-0.2%).
- Method notes: `perf record -e instructions:u -c 400000` on a frame-pointer `fast` build, attributed by source
  line and by instruction (scratch scripts under `target/prof`, not committed), and `perf stat` instruction
  counts, which do not depend on the load; the box was shared, so cycles and fights/s were compared ABAB.
- Left: SmallVec bookkeeping (about 6% of instructions), the per-fight state clone (3%), the listener rebuild on
  card and creature changes (1.4%), `is_ending` (about 2%, a scan of the enemies each call).

## Whole runs round 2: `Run::apply` without a copy, compact maps (task PF10)
Every step keeps the digests (fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, 2,000 runs `af6c19933b438172`,
2,000 runs of every character `125847415c9a423a`, 32,000 runs on 16 threads `bfb062b545664dda`) and the map digest
tests. At the end `tools/regress.sh 5` (6,175 ok), `tools/regress_runs.sh 8` (497 ok), the strong sweep (table
identical to the base's: 1,000 runs, 615 victories, 864,673 actions) and `tools/replay_history.py --db` (8,046
runs; summary and per-run details byte-identical to the base's) pass. Steps measured by `perf stat -e
instructions:u` (10,000 runs of every character, pinned), since other workers moved wall-clock rates by ±10%.

| Step | Change | instructions, 10k runs |
|---|---|---:|
| base `174bfc2` | | 14.67G |
| 1 | `MapPoint` is `Copy`: `parents`/`children` are `Neighbors` (up to 7 coords as bytes, inline) instead of two `Vec`s per point; `ActMap::point` binary-searches | 12.76G |
| 2 | `Run::apply` rolls back by checkpoint and replay instead of applying each out-of-combat action to a copy (below) | 12.06G (no-copy bound 11.95G) |
| 3 | map builder: adjacency and row lists in `SmallVec<[Id; 7]>`, `all_points` inline, points emitted row by row instead of sorted, `spread_adjacent_points`' allowed columns as a bitmask, one- and two-digit segment-key numbers written directly (Act 1 map −14% instructions) | 11.70G |

- **Rollback (`run/rollback.rs`).** A run cloned itself (about 52 allocations, 22 KB: relic bags 10 KB, map 3 KB,
  act room lists 2.5 KB) for every action outside combat, because such an action can fail after changing the run
  (an unported card found when the fight is set up, a map the game throws on) and a rejected action must change
  nothing. Now `Run` keeps a `Journal`: a checkpoint (a copy taken every 64 actions) and the actions applied
  since, combat ones included. A rejection restores the checkpoint and replays the log; the engine is
  deterministic, so that is the run as it was. Rejections are rare (one in 4,000 random runs). Every change
  outside `apply` (`loadout_state`, `loadout_relic`, `debug_enter_event`, `debug_enter_act`, `with_events`,
  `in_test_mode`) drops the journal; `Clone` gives an empty one, so a copied run takes its own checkpoint on its
  first out-of-combat action (a one-action lookahead costs what it did). Debug and `regress` builds still apply
  every out-of-combat action on a copy as well and assert that a rejection restored exactly that copy, so the
  regressions and the sweep check the replay. A unit test that sets a run's fields directly must call
  `forget_journal` (two did).
- **Final**, same session, interleaved (Necrobinder 2,000 runs on `taskset -c 5`, three rounds; 32,000 runs on 16
  threads, two rounds; load 2-4):

  | Variant | runs/s 1 core | runs/s 16 threads |
  |---|---:|---:|
  | base LTO | 6,450 | 63.6k |
  | PF10 LTO | 7,360 | 76.7k |
  | base PGO | 7,170 | 72.7k |
  | PF10 PGO | 8,760 | 88.9k |

  Fights are unchanged (59.7k LTO, 85.5k PGO fights/s in `cargo xtask bench`).
- Where a random run's time goes now (scratch timing of the bench loop): `Run::new` about a fifth (the Act 1 map
  ~22 µs of ~28 µs: pruning 10, post-processing 4.8 before step 3, path walk 3.1, point types 2.7, output
  1.8; relic bags 2.1, mostly their ~240 shuffle draws; rooms 2.7), combat actions under half, `ChooseNode` ~5 µs
  (mostly `Combat::new`, combat code), `legal_actions` ~7%.
- Dropped (no measurable change in instructions): skipping the rewards sort when already in order, an `IdTable`
  for `room_type_of_encounter`. A flat profile under fat LTO attributed 2.5% and 1.4% to them; with heavy
  inlining, `addr2line` line tables and frame-pointer call chains misattribute, so every step here was checked by
  instruction counts. Local `perf report` does not symbolize these binaries; resolve sample addresses with
  `addr2line -i` against the binary's load segment instead.
- Left: the map's segment pruning (PF7's index; ~10 µs, now the largest generation cost), `Combat::new` and the
  combat snapshot clone (combat code, PF5).

## Toolchain and whole runs (task PF7)
Every step keeps the digests (fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`, 2,000 runs `af6c19933b438172`),
the map digest tests (200 and 2,000 seeds) and the from-scratch segment oracle; at the end `tools/regress.sh 5`
(6,175 ok), `tools/regress_runs.sh 8` (497 ok) and the strong sweep (1,000 runs, 615 victories, 864,673 actions,
identical table) pass. Measured `taskset -c 3`, best of 3-4 with `cycles:u` alongside, at load 2-6.

**Whole runs** (`--mode runs --runs 2000`, LTO; random play dies on floor 1-2, so `Run::new`'s Act 1 map dominated):

| Step | Change | runs/s |
|---|---|---:|
| base `1268df2` | | 1,300 |
| 1 | `SegmentIndex::groups` fills a run-id list instead of copying segments; dead runs and keys dropped for good; keys whose runs all overlap dropped at build | 2,800 |
| 2 | generated `Kind::from_id` was a linear scan of `ALL` comparing strings (also `RunCard::to_combat`, `PoolCard::kind`); first a binary search | 3,160 |
| 3 | runs keep no point lists (route rebuilt from first point, rank offset and length; overlap by point bitmask) | 3,440 |
| 4 | the keys that can group found by a paired route search (two routes in lockstep through distinct points of equal type until they meet) instead of listing all ~2,900 segments of ~1,250 keys per map; ~6 keys per map can group | 5,620 |
| 5 | segment liveness read off the live graph (start reaches the first point, edges remain, last point reaches the boss): no arrival bases, ranks or bitset | 6,060 |
| 6 | `from_id` through `content::IdTable` (FNV-1a, linear probing): the relic bag culls every relic through it on each pull | 6,350 |
| 7 | reachability by row order instead of a memoized recursion | 6,590 |

The 2,000-seed map digest test went from 6.7 s to 1.3 s (release). Left in whole runs (profile of step 7, all
characters): `Run::apply` clones the run for every non-combat action (~8%; a third is the map's per-point
`parents`/`children` `Vec`s, which are public API used by the TUI and `sts2-diff`; inline arrays would remove it),
`Combat::run`'s snapshot clone (combat, PF5), card reward and room generation (~3% each), the relic models'
`is_allowed` on each bag pull (~2%).

**Build variants** (`cargo xtask bench [--native] [--pgo] [--bolt]`, final code; fights 2200x50, runs 2,000
Necrobinder; all cores = 16 threads, sum of per-thread rates, 35,200 seeds / 32,000 runs, load 5-13, ±8%):

| Variant | fights/s 1 core | runs/s 1 core | fights/s 16 threads | runs/s 16 threads |
|---|---:|---:|---:|---:|
| base LTO (`1268df2`) | 57,900 | 1,300 | 575k | 12.5k |
| LTO | 59,600 | 6,480 | 540k | 66k |
| LTO + BOLT | 64,300 | 6,980 | | |
| PGO | 86,800 | 7,260 | 831k | 75k |
| PGO + BOLT | 86,200 | 7,330 | 842k | 77k |
| native + PGO + BOLT | 86,400 | 7,570 | 840k | 78k |

- Kept: `panic = "abort"` in `[profile.fast]` (with a matched PGO profile 6% fewer cycles on fights; LTO alone ±2%,
  as PF4 found; only `sts2-bench` uses the profile, the sweep's `catch_unwind` runs in `regress`). `--bolt`
  (llvm-bolt instrumentation, `-reorder-blocks=ext-tsp -reorder-functions=cdsort -split-functions`, on a binary
  linked with `--emit-relocs`; Ubuntu `apt install bolt-22`): +8% fights and runs on LTO, +1-3% on PGO.
  `--native` (`-Ctarget-cpu=native`, Zen 4 with AVX-512): 0-3%, within noise, kept opt-in.
- PGO training set: fights-only and the current set (220x10 fights + 100 Necrobinder runs) are equal on fights;
  adding 1,000 runs of every character cost 4% on fights and gained 3% only on other characters' runs. Kept.
- Dropped: thread pinning (`core_affinity`, one worker per CPU): +1%, within noise, not worth the dependency.
  `MIMALLOC_ALLOW_LARGE_OS_PAGES=1` / `MIMALLOC_LARGE_OS_PAGES=1` (THP is `madvise` here): no change; the working
  set is small. SMT: 8 threads one per physical core (`taskset -c 0,2,..,14`) give 563k fights/s and 50k runs/s
  against 16 threads' ~840k and ~77k, so SMT adds about 45%.
- Interleave builds when timing all cores: other workers' load moved single rounds by 8-10% (four LTO builds that
  differ only in run code measured 504k-575k apart, and 530-540k each when alternated).

## Beat spire-mentor (task PF4, perf-plan items 6 and 8, allocator)
W1 as above; every step keeps the digests (fights `ad55be60f5bd8a71`, runs `1febef7e3b528fe5`), and at the end
`tools/regress.sh 5` (6,175 runs), `tools/regress_runs.sh 8` (497) and the strong sweep (1,000 runs, 615 victories,
864,673 actions, identical to `main`) pass. Instructions per action from `perf stat -e instructions:u`,
2200 x 50 pinned (setup included, so a little above the timed loop).

| Step | Change | LTO instructions/action |
|---|---|---:|
| base `22685df` | | 14,978 |
| 1 | mimalloc as the bench's global allocator | 14,540 |
| 2 | `Decimal` fast paths for integral values (no `i128` division by 10^0, no scale alignment) | 13,302 |
| 3 | combat-end guard: Minion compared by kind, not a dyn call per power; `starting` read first; energy-cost guard only with a listener | 12,256 |
| 4-5 | `smallvec` for the turn flow's short lists, `damage`/`attack`/`draw` results and the dispatch snapshot; `legal_actions_into` | 11,452 |
| 6 | `CanPlay` and the target type once per hand card; `CardKind::id_rank` (build.rs) for `CardModel.CompareTo`; pile removal by position | 10,414 |
| 7 | `pass!` tests `Engine::lacks` inline: a hook nobody overrides is no call (about 40 per action) | 9,939 |
| 8-10 | fewer small libc `memcpy` calls; `may_prompt` flags from tables; single-card pile moves skip the batch buffer | 9,940 |
| 11 | `Flow<T> = Result<T, Box<Interrupt>>`: a `Flow` fits a register instead of a 72-byte result per `?` | 9,810 |
| 12-14 | energy/star cost, damage, HP-loss, redirect and block folds return early without a listener; `player_creatures` without a scan | 9,377 |

Final, `taskset -c 9`, interleaved with spire-mentor `8e0e917` at load 12-16 (the first of two rounds; the second was
disturbed by other workers): `main` LTO 39,300, PF4 LTO 60,100, PF4 PGO 84,600, spire-mentor 72,400 fights/s.
Cycles per action (less load-sensitive): LTO 4,074, PGO 2,761, spire-mentor 3,227; instructions per action LTO 9,377,
PGO 6,791, spire-mentor 6,613. All 16 threads as 16 pinned full-length processes (spire-mentor's method; `--threads
all` slices the seeds too thinly to time), load 12: LTO 502k, PGO 751k, spire-mentor 571k fights/s. So PGO is ahead
by about 1.17x on one core and 1.3x on all cores; LTO alone is about 0.8x (1 core) and 0.88x (all cores).

- Tried and dropped (no gain): `#[inline]` on `record` and the small `Hooked` folds, a two-pointer `Hooked`
  (+3% instructions), `panic = "abort"`, `-inline-threshold=1000` (-8% instructions, +4% speed).
- What is left for LTO alone: the profile is flat. PGO removes 28% of the instructions by inlining and
  devirtualizing; by hand that means static per-kind tables for the card model's no-argument methods
  (`target_in`, `is_playable`, `card_type` are dyn calls per hand card in `legal_actions`, about 1,200
  instructions per call), `is_ending` (14 per action), `HistoryEntry` (80 bytes, a store-forwarding stall when
  copied in `record`), and item 9 (`Card` is 272 bytes; piles as `SmallVec`).
- Tooling notes: the scratch LD_PRELOAD shims that counted malloc and memcpy call sites per action, and
  `instructions:u` sampling, are how the steps were picked. A pre-warmed `target/` copied from another checkout
  keeps that checkout's `env!("CARGO_MANIFEST_DIR")` in `xtask` (so `--pgo` wrote to the other checkout's
  `target/pgo`) and a stale `sts2-macros` in `target/regress`; `touch` the sources once to rebuild them.

## Combat listeners and build (task PF3, perf-plan items 1-3, 5, 7)
W1 (`cargo xtask bench --mode fights`, 2200 x 50). Every step keeps the digests: fights `ad55be60f5bd8a71`
(35,868 wins, 2,412,377 actions), runs `1febef7e3b528fe5`; `tools/regress.sh 5`, `tools/regress_runs.sh 8`
and the sweep (1000 runs, 615 victories, 864,656 actions) are unchanged.

| Step | Change | fights/s, 1 core | instructions/action |
|---|---|---:|---:|
| base `e61c830` | | 4,600 | 103,000 |
| 1 | one listener list per `legal_actions` / `check_play` (`can_play_with`) | 9,300 | 42,700 |
| 2 | hookless listeners dropped from the list; `Hooked::hooks` (the union) skips absent hooks | 14,700 | 26,300 |
| 3 | `Combat` keeps the listener cache across steps and refreshes it for `legal_actions`; a card move only invalidates it if the card, its affliction or its enchantment listens (`Engine::cards_moved`) | 23,000 | 17,000 |
| 5 | `#[sts2_macros::hooks]` emits `X::HOOKS`; `build.rs` makes `Kind::hooks` tables; the listener walk reads them off the objects (no `bind`) | 24,100 | 16,500 |
| 7a | `[profile.fast]`: fat LTO, `codegen-units = 1` (`cargo xtask bench`) | 26,800 | 15,900 |
| 7b | PGO (`cargo xtask bench --pgo`) | 39,000 | 12,000 |
| 6 | `legal_actions` pre-sized, `card_targets` an iterator, no `Vec` in `end_of_turn_cleanup` (LTO / PGO) | 38,600 / 56,400 (load 7) | 14,650 / 10,750 |

- Measured ABAB (two rounds, mean) on `taskset -c 9` at load 11-12, so the wall-clock rates are lower than on an
  idle box: the base measured 6,300 at load 5. Instruction counts (`perf stat -e instructions:u`) do not depend
  on load. All 16 threads at load 11: base 42k, LTO 248k, PGO 350k fights/s (sum of per-thread rates).
- Final, at load 7-14 (ABAB, `taskset -c 9`): base 5,500, step 7a (LTO) 35,100, step 6 LTO 38,600, step 6 PGO
  56,400 fights/s. All 16 threads at load 12: base 40k, LTO 271k, PGO 438k.
- spire-mentor `8e0e917` interleaved on the same core: 72,200 fights/s and 6,600 instructions/decision (its bench
  is fat LTO, no PGO). The gap is 1.28x with PGO and 1.87x with LTO alone (11.5x before PF3).
- `cargo xtask bench` builds `sts2-bench` in the `fast` profile (`target/fast/`, about 1.5 min from clean
  workspace crates). `--pgo` builds an instrumented binary (`target/pgo/gen`), trains it on 220 x 10 fights and
  100 runs from seed 1,000,000 (not the measured seeds), merges with `llvm-profdata` (rustup component
  `llvm-tools`) and benches the `-Cprofile-use` build (`target/pgo/use`): about 4 minutes.
- The cache stays honest through the existing debug oracle: in debug and `regress` builds every use of a cached
  list (`ListenerCache::get`) is compared with a fresh one. Unit tests change `Combat::state` directly between
  steps, so under `cfg(test)` `Combat` invalidates the cache before each step and does not reuse it in
  `legal_actions` (15 tests would trip the oracle otherwise).
- `the_listener_list_is_the_full_list_without_hookless_listeners` checks the sparse list against the full walk.
- The sweep (`regress` profile) went from 58 to 166 runs/s.
- Next (profile of step 5 with LTO-free frame pointers): the listener list no longer shows; time is spread over
  the turn flow (`end_player_turn` 41%, `start_turn` 36%, card play 22%, `legal_actions` 17%) and libc malloc
  and free (17% self). Candidates: item 8 (hook-presence words so `is_guard_closed` / `is_ending` and absent
  hooks cost an AND; `is_ending_unless` alone is 5%), more of item 6 (`MoveMachine::log` regrows after every
  clone, `CombatHistory::push` 3%, the `draw` and `shuffle` `Vec`s; an inline `dispatch` snapshot measured
  only -2% cycles), then a faster allocator or `SmallVec` piles (needs the dependency approved) and item 9.

## Map generation (task PF2, perf-plan item 4)
- Pruning's duplicate-segment search is incremental (`run/map/segments.rs`, adapted from spire-mentor's
  `SegmentIndex`, MIT, see `NOTICE`). It indexes the paths once per pruning loop as rank intervals, retires
  the paths through each removed point or edge from a bitset, and interns segment keys as packed `u128`s.
  The ASCII key is only written to sort the keys that have two or more runs.
- Act 1 map: about 19 ms → 0.71 ms (pinned, load ~12). The index build is about 350 µs; each refresh after a prune is about 24 µs.
- `cargo xtask bench --mode runs` (200 runs, `taskset -c 9`, load 9-25): 36 → about 715 runs/s (20×).
  Digests unchanged: fights `ad55be60f5bd8a71`, runs `af4f5b489b2a7014` (the runs digest had already
  moved from PF1's `4a353e75d1d6e2b9` before PF2).
- Pinned by `run::map::tests::map_digest_is_unchanged` (200 seeds × acts 0-2 × A0/A10 plus Spoils, maps
  and final map-stream state, recorded on the old code; `--ignored` runs 2,000 seeds), and by
  `segment_index_matches_the_from_scratch_search` (the old search as an oracle after every prune).

Workload: `crates/sts2-core/examples/bench.rs` (starter deck vs FuzzyWurmCrawlerWeak, random policy, release).

## Method
- `perf` is not installed and `perf_event_paranoid=2` with no sudo; `samply` was built but refuses to run. So the
  profile comes from a scratch sampler (`target/prof`, not committed): `SIGPROF` every ~4 ms of CPU, frame-pointer
  walk (`-C force-frame-pointers=yes`), `backtrace` crate symbolisation with inlined frames, 150k combats, 8.9k samples.
- The machine was loaded (load average ~30 on 16 cores), so only **relative shares** are meaningful. Sampling noise
  is about +/-0.5 percentage points. The scratch build also has a counting global allocator (about 2% overhead,
  visible as `atomic_add`).
- Throughput: **2.0k-3.2k combats/s** under that load (the brief's idle figure is ~10k). **24.75 actions per combat**
  on average (3943 of 5000 combats won).
- Wall-clock split with `Instant`: `Combat::new` ~5%, `legal_actions` + pick ~42%, `apply` ~48%.

## Allocations per combat (counting allocator)
1563 allocs and 336 KB per combat, 63 allocs per action. `legal_actions` 965 (39 per call), `apply` 572 (23 per call),
`Combat::new` 61. Nearly all are `Vec<Entry>` growth (1, 4, 8, 16, 32 elements) inside `with_listeners`.

## Top 15 (inclusive % of samples; self % where it is a leaf; nested rows overlap)
| # | Concept | Incl | Self | Where |
|---|---------|-----:|-----:|-------|
| 1 | `Combat::apply` (whole engine step) | 48 | - | `combat/mod.rs:449` |
| 2 | `legal_actions` (everything is `check_play`) | 41 | - | `combat/mod.rs:413`, `check_play` `:683` |
| 3 | Listener-list rebuild, all callers (`listener_entries`) | 47 | 3 | `combat/hooks.rs:1190` |
| 4 | Fresh list per hook query (`with_listeners`): `energy_cost` 17, `star_cost` 14, `should_play` 14 | 45 | - | `hooks.rs:1233`; calls at `mod.rs:694/697/700` |
| 5 | Cached-list rebuild after every pile or power change (`Engine::hooked`) | 13 | - | `combat/engine.rs:333` |
| 6 | Hook dispatch (`Engine::dispatch`: snapshot `Vec<Listener>`, re-check) | 21 | 1 | `hooks.rs:2012` (snapshot collect 5.5% at `:2023`) |
| 7 | Per-listener model lookup: `bind` 7.7, `Bound::hooks` 4.3, `HookSet::contains` 4.4, `CardKind::model`/`id` 5, `PowerKind::model` 1.3 | ~25 | 23 | `hooks.rs:1243`, `:1109`, `:1090`; `CardKind::model`, macro-generated at `content/cards/mod.rs:256` |
| 8 | `Vec<Entry>` growth (`grow_one`, `realloc`) | 17-20 | - | `hooks.rs:1195` (push), `:1235` (`Vec::new()`) |
| 9 | libc malloc/free/realloc internals (unsymbolised `?`, `realloc`, `free`) | ~24 | 24 | via #8 and `dispatch` |
| 10 | Turn flow: `end_player_turn` 26 (incl. enemy turn + next `start_turn` 21) | 26 | - | `combat/engine.rs` |
| 11 | Card play: `play_card_action` 15, `on_play_wrapper` 11, `content::cards::on_play` 5 | 15 | - | `engine.rs` |
| 12 | Pile moves + `after_card_changed_piles` dispatch (`draw` 10) | 11 | - | `engine.rs` |
| 13 | End-of-combat guards (`is_ending`, `is_guard_closed`, `is_over_or_ending`) | 3.4 | 1 | `hooks.rs:1410`, `:1422` |
| 14 | `Combat::new` / setup, incl. `start_combat` | 3 | - | `combat/mod.rs:360` |
| 15 | Shuffle 2.3, decimal (`to_int_truncating`) ~1-2, history push 0.8, RNG <0.5, policy + loop ~5 | ~10 | - | `shuffle.rs`, `decimal.rs`, `combat/history.rs:174` |

Not hot, despite the usual suspects: decimal emulation (~1-2%), history logging (0.8%), RNG (<0.5%, shuffle 2.3%),
cloning (no `clone` frames at all), the policy (<1%). The bench's per-combat `format!` seed is lost in the noise.

## Diagnosis
About 58% of the time is spent rebuilding the listener list (`Vec<Entry>`: ~20 entries, i.e. every card in every pile,
all powers, the monster) and growing it from empty, then scanning it. `legal_actions` builds it **three times per
candidate play** (energy, stars, should_play) for each of ~5 hand cards x targets, every step, though nothing changes
between those calls. The rebuild itself is dominated by `bind` + `hooks()` + the `CardKind::model` match per object.
Roughly 25% of CPU is malloc/free, nearly all from these short-lived vectors.

## Speedup ideas, ranked (gain = share of current runtime removed; risk = parity)
1. **One listener list per `legal_actions`.** Build `Hooked` once, pass it to a `check_play_with(&Hooked, ..)`; use
   it for `energy_cost`/`star_cost`/`should_play` (`mod.rs:694-700`, `hooks.rs:1744/1794/1901`). Gain ~30-35% (about
   1.5x throughput). Parity risk: very low (`&self`, read-only, same listener order, same results).
2. **Per-kind constant `HookSet` table** so `bind`/`Bound::hooks`/`CardKind::model` is an array lookup instead of a
   giant match plus `dyn` call (`hooks.rs:1109`, `content/cards/mod.rs:256`, `content/powers/mod.rs:174`). Gain ~10-15% (shrinks #7).
   Risk: low; a test asserting table == `model().hooks()` for every kind pins it.
3. **Preallocate the entry vector** (`Vec::with_capacity(32)`) or reuse a scratch buffer in `with_listeners`
   (`hooks.rs:1235`) and `Engine::hooked`. Gain ~8-12% on its own, ~4% after idea 1. Risk: none.
4. **Release profile**: `lto = "fat"`, `codegen-units = 1` (no `[profile.release]` exists today, see `Cargo.toml`).
   Gain ~5-15% (inlining `bind`/`model` across the `content` modules). Risk: none for integer logic; the f64 RNG and
   monster-weight code stays IEEE-exact. Check that `cargo xtask gate` still passes.
5. **Avoid the dispatch snapshot allocation**: collect into a `SmallVec`/stack array or an `Engine`-owned scratch
   (`hooks.rs:2023-2028`) while keeping snapshot-before-call semantics. Gain ~4-5%. Risk: low (re-entrant
   dispatch must not share one buffer; use a stack of buffers or `SmallVec`).
6. **Keep the cached list valid across pile moves** (`Engine::hooked`, `engine.rs:333`; 13%): patch it incrementally
   when a card moves piles instead of marking it stale, or drop entries whose `HookSet` is empty so the list is a few
   entries. Gain ~8-10%. Risk: **medium** - listener order is `LISTENER_ORDER` of piles (`state.rs:63`) and must match
   the game exactly; needs the existing `debug_assertions` equality check at `engine.rs:339` kept as the oracle.
7. **Faster allocator** (e.g. mimalloc) or arena for the remaining per-step Vecs. Gain ~5-10% after ideas 1 and 3.
   Risk: none for parity; adds a dependency, so check the project's dependency policy first.
8. **`legal_actions` own allocations**: `card_targets` returns a `Vec` per card (`mod.rs:611`) and the result `Vec`
   grows from empty (`mod.rs:424`); return a small array or pre-size by hand size. Gain ~2%. Risk: none.

Combined 1+2+3+4 should give roughly 2.5-3x; 6 and 7 are the next tier if more is needed.
