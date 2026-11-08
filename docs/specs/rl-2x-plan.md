# RL 2× plan: staged migration of sts2-core to the kernel-shaped generalist core (2026-10-24)

Scope: design only, from the detached worktree `~/spirebot-worktrees/FB2` (a copy of `main` at `37687cb`).
Nothing committed. Scratch prototypes measured here and left uncommitted in FB2: a fixed-array `Pile`
(`crates/sts2-core/src/combat/state.rs`), an RL-shaped bench mode (`crates/sts2-bench/src/rl.rs`, `--mode rl`),
a `BENCH_CHAR` switch for the fight workload's character, and a one-line adaptation in
`crates/sts2-kernel/src/project.rs`. Callgrind outputs are in `FB2/target/scratch/*.cg`.

Inputs: `docs/reviews/perf-architecture.md` (the FB1 study: front-end bound, IPC 2.56, 5,540 instructions per
action; kernel 2,400 at IPC 3.34), `docs/perf.md` (PF1-PF14), `docs/specs/copy-state-plan.md`,
`docs/kernel-prototype.md`, `docs/specs/content-api.md`, `docs/specs/hooks.md`, and the combat code. PF15 (PGO
by default, a `may_prompt` cache, one non-generic `dispatch` body) is treated as merged.

## 0. Verdict

- **On the RL-shaped workload, 2× is realistic with margin.** Today a naive RL step (legal mask + observation +
  engine step) costs 8,950 instructions, of which 3,400 are extraction that the kernel-shaped state turns into a
  memcpy and a bitset write. The engine stages below take the step itself from ~5,500 to ~3,500-3,900. Expect
  2.3-2.6× on instructions, and more on wall time because the footprint shrinks.
- **On W1 (Ironclad starter, LTO, one core) 2× over the 101k LTO baseline is at the edge:** the committed stages
  reach about 1.7× (3,900 instructions per action, ~1,250 cycles, ~175k fights/s); the stretch items in stage 4
  and 6 are needed for 2.0× (~3,400 instructions, ~1,100 cycles, ~200k). **2× over the PGO build (139k → 278k) is
  not realistic for a generalist engine with history, events and replay**; expect 230-250k PGO (1.7-1.8×). The
  kernel's 720 cycles per action is the shape's floor for 38 encounters without history.
- The biggest parity risk is not the layout but the **dispatch rewrite (stage 1)**: listener *order* and the
  mid-pass membership re-check are what every digest and referee trace depend on, and the rewrite deletes the
  cached entry list that today's debug oracle checks. The plan keeps the old walk as the oracle for one release,
  adds a lockstep old-engine-vs-new-engine harness built once in stage 0 and run on every stage, and keeps the
  existing gate (digests, `regress.sh`, `regress_runs.sh`, strong sweep byte-identical, `replay_history.py`,
  the kernel differential test).

## 1. Measurements made for this plan

Instruction counts are `perf stat -e instructions:u` over the whole process divided by actions (load-independent;
the box ran at load 9-10 all day, so wall-clock rates below are not usable: the same binary measured 66k, 80k and
90k fights/s in consecutive runs). Callgrind on a frame-pointer `fast` build for attribution.

| workload (1,100 seeds × 50) | build | instr/action | note |
|---|---|---:|---|
| W1 fights, Ironclad | `main` | **5,541** | matches the FB1 study (5,538) |
| W1 fights, Necrobinder (`BENCH_CHAR`) | `main`+piles | 5,991 | +7% over Ironclad on the same build |
| W1 fights, Silent | `main`+piles | 8,849 | +58%: mimalloc is 18% of it (1,150 instr/action in `malloc` alone), called from inside the attack path of `StrikeSilent::on_play` (85.6M over 59,901 plays) and `Neutralize::on_play`; not the replay snapshot (`CombatState::clone` is 1.1%); the Strike bodies are byte-identical to Ironclad's. **Open profiling item, stage 0.** |
| LA (`--lookahead`, 200 seeds) | `main` | 31,805 | |
| RL step, 1 env, obs + mask, no intents | `main` | **8,947** | prototype `--mode rl` |
| RL step, no observation (mask + policy only) | `main` | 6,699 | so the observation is ~2,250 and mask+policy+decode ~1,160 |
| RL step with naive intents (`--intents`) | `main` | 21,650 | `intent_damage` clones the state and performs the move on a scratch copy per enemy per step: 12,700 per action |
| RL step, 64 / 256 envs round-robin | `main` | 8,925 / 8,929 | instructions identical; cycles +16% at 64 envs on the loaded box (not conclusive, within today's noise) |
| W1 with fixed-array piles, cap 64, `u8` len, panic on overflow | FB2 proto | 5,616 (+1.4%) | digest unchanged; LA +2.8% |
| W1 with fixed-array piles, cap 256 (`u8` len makes the prefix slice check provable) | FB2 proto | 5,590 (+0.9%) | LA +6.1% (5 KB more per copy) |

Callgrind diff of the cap-64 pile build against `main` on W1 (per action): `remove_from_pile` went out of line
(+92), `memcpy` +56, `draw` +54; `on_play_wrapper` −40, `move_to_pile` −31, allocation (`finish_grow`, mimalloc)
−40. **A fixed array alone is neutral on W1**: PF9's +4-6% was SmallVec's inline/heap branch, and without it the
array neither wins nor loses; its value is in copies (LA, RL reset, replay snapshots, observation views) and only
if it stays small.

High-water marks over W1 (110,000 fights, random play): 40 cards in combat, 6 creatures, 3 powers per creature,
7 removed powers per creature, hand 5, largest pile 25. These are lower bounds; stage 0 surveys the strong sweep.

Kernel reference points (from `docs/kernel-prototype.md`, same box): 2,403 instructions, 719 cycles per action,
IPC 3.34, 3.4 L2 code fills per action; LA 11,400 per action.

## 2. Target architecture

One engine (`sts2-core`), all content unchanged in behaviour. The shape it converges to:

1. **State = `Hot` + `Cold`.** `Hot` is one contiguous, fixed-capacity, heap-free struct (target 4-8 KB): scalars,
   the run RNG streams, creatures with their power lists, the hot part of every card, the five piles, the turn's
   history list, allies/enemies, listener-presence unions (below). Capacities come from the stage-0 survey (×2,
   rounded to a power of two). `Cold` keeps `Vec`s for what is rare or unbounded by the game: `power_data`,
   `cost_lists`, `removed_powers` beyond the inline slots, `deck`, `extra_rewards`, `removed_from_deck`,
   `escaped`, `spawned_enemies`, events, the pending prompt. **Spill, not refusal:** when a hot list would exceed
   its capacity the whole `Hot` is re-laid out with that region doubled (it is one slab of `u16`/`u32` words with a
   region table: offset, len, cap per list), so the engine code is the same before and after, and no fight is
   refused. A spilled state copies as one larger memcpy. This differs from PF9's slab (which shifted cards between
   adjacent ranges on every move): regions keep slack, and only an overflow re-lays out.
2. **Ids narrow**: `CardId`/`CreatureId`/`PowerId` as `u16` (the id tables stay index-stable). Pile entries `u16`.
3. **Hook presence as maintained unions, no entry list**: `Creature::hooks: HookSet` (union of its powers' `HOOKS`
   and its monster's), `Player::item_hooks` (relics, potions), `CombatState::card_hooks` (union over every card in a
   combat pile plus its affliction and enchantment; maintained by counts per hook so it is exact, updated only
   when a card enters or leaves combat or gains an affliction or enchantment, never on a pile move, because every
   pile listens). `lacks(hook)` is one test of the OR. A pass walks the state in the game's listener order
   (`CombatState.cs:411-470`), skipping creatures whose union lacks the hook and skipping the card walk when
   `card_hooks` lacks it, snapshotting the matches as today and re-checking `contains_listener` before each call.
   The `ListenerCache`, `Entry`, `bind`, `cards_moved`, `card_listens` and all in-place maintenance go away.
4. **Piles**: draw pile stored bottom-first (a draw is a pop) behind the same API (`Player::pile(Draw)` keeps
   returning top-first through an iterator view); batch moves check once per batch.
5. **Dispatch**: one non-generic body per hook family (PF15), `#[cold]` interrupts, events built only when
   recorded. Content keeps `#[sts2_macros::hooks] impl Hooks for X` and the `CardModel`/`PowerModel` traits as is.
6. **Numbers**: damage, HP-loss and block folds run on integers when every modifier is in the kernel's proven form
   (additive integers, percentage factors, cap, one truncation; `crates/sts2-kernel/src/num.rs`) and fall back to
   `Decimal` otherwise; debug builds compute both and assert equality.
7. **History, events, replay stay game state**: history is read back by content (`history_this_turn`,
   `lost_hp_this_turn`, `last_attack_played_in_turn`); it moves into `Hot` as a fixed-capacity turn list with the
   same spill rule. Events stay an optional sink. The replay snapshot (model C) becomes one memcpy of `Hot`.

## 3. Workloads and bench modes

Existing: W1 (`--mode fights`, Ironclad starter × every Overgrowth encounter, 2,200 × 50, digest
`ad55be60f5bd8a71`), LA (`--lookahead`, `85229a762f9361e1` at 1,000 seeds), runs (`1febef7e3b528fe5`,
`af6c19933b438172`, `125847415c9a423a`, `bfb062b545664dda`).

Add in stage 0:

- **W1-all**: W1 for each of the five starter decks (`--character`), reported per character. Silent's deck is
  58% more expensive per action today; Necrobinder's prompts; neither is visible in W1.
- **RL bench mode (`--mode rl`)**, the workload PufferLib would drive, defined so a number means one thing:
  - *Envs*: `--envs N` fights per thread (default 64; also report 1 and 256), stepped round-robin, one action per
    env per round, as a vectorised env does. `--threads T` as today (sum of per-thread rates; PF1's convention).
  - *Episode*: a fight from a saved start snapshot. Starts come from a corpus (stage 7): every character, both
    Act 1s, floors 1-15, decks and relics from the sweep's greedy runs; until then, W1-all's starts. A finished
    fight (won, lost, unsupported, or the 10,000-action cap) is reset from the next start with `clone_from`
    (one memcpy after stage 4); the reset is timed.
  - *Step*: for each env in turn: write the legal-action bitset, write the observation (section 7.1, intents
    included), pick a uniform random legal action from the bitset (the policy's RNG is SplitMix64 seeded by the
    start's seed and repetition, never an engine stream), decode it, apply it. Everything but the policy's draw
    is timed. Decisions (card choices) are steps like any other.
  - *Output*: `rl-fights/s`, `actions/s`, `ns/action/thread`, the digest (FNV over seed, won, actions per fight;
    independent of `--envs` and `--threads`), and `obs bytes/step`. Instruction counts per action from
    `perf stat` are the gate number; wall time on a quiet pinned core second (ABAB, median).
  - *Variants*: `--no-obs` (mask only), `--no-intents`, `--envs`, to attribute cost.
  - The prototype in FB2 is this minus the corpus and the intents cache; its numbers are the table above.
- **Capacity survey** (`cargo xtask sweep --capacities`): high-water marks of every hot list over the strong sweep
  (5,000 greedy runs × five characters × three acts): cards in combat, creatures, powers and removed powers per
  creature, hand and piles, history entries per turn, relics, potions, orbs, `cost_lists`, `power_data`, pending
  options. Fixes the capacities of stage 4 and the observation's slot counts.

## 4. Stages

Each stage is one branch merged into `main` with the full check list of section 5 green, measured by
instructions per action on W1, W1-all, LA, runs and RL (ABAB wall time on a quiet core as the second number),
digests identical. The rollback point of every stage is the merge commit; stages 1-3 and 5-6 are independent of
each other, stage 4 depends on 1 and 3, stages 7-9 depend on 0 only (and gain from 4 automatically). Effort is
worker-days for one core engineer unless stated. "Gain" is W1 instructions per action unless stated; the budget
is in section 8.

### Stage 0: oracles, workloads and the open profile (4 days, tooling + core)
- **What changes**: (a) the lockstep harness (section 5.2); (b) the RL bench mode, W1-all and the capacity
  survey (section 3), with the FB2 prototype as the starting point; (c) the Silent allocation profile: find and
  remove the ~1,400 instructions of `malloc` per Strike play in Silent fights (callgrind puts it inside the attack
  path under `on_play`; Ironclad's identical Strike has none; candidates are the modifier lists, a target list
  spilling its SmallVec, or an `Option<Box>`), since any deck with debuffs likely pays it; (d) a `canonical_digest`
  on `CombatState` (hp, block, energy, stars, pile contents by card kind and upgrade, powers by kind and amount,
  RNG counters per stream, history counts) for the lockstep harness and the RL digest.
- **Content files**: none.
- **Gain**: none on W1 by design; the Silent item is likely −10 to −15% on Silent's W1 and on every debuff deck.
- **Parity proof**: the full list (section 5.1). The harness proves itself by comparing `main` with `main`.
- **Rollback**: trivial (tooling). **Effort**: 2 days harness, 1 day bench and survey, 1 day profile item.
- **Done (RL0, 2026-10-24; details and numbers in `docs/perf.md`, "Stage 0 of the RL 2× plan").**
  `cargo xtask lockstep` (50,000 fights over 7,219 specs plus 100 greedy strong runs, 0 differences against the
  base, ~35 s on 8 jobs; bump the two `rev`s in `crates/sts2-lockstep/Cargo.toml` at every merge);
  `sts2_core::digest::canonical_digest`; `--mode rl` (digest `372d441d8132f8de`, 25,503 instructions per step of
  which 74% is extraction, intents alone 52%); W1-all (`--character`); the capacity survey (`sweep --capacities`,
  `STS2_CAPACITIES`), with the capacities it suggests for stage 4 in `docs/perf.md`. **Item (c) correction:** the
  Silent cost is not in the attack path but `Combat::run`'s replay snapshot, a full `CombatState::clone` and drop
  before every step whenever `may_prompt` holds (Survivor is in Silent's starter deck): 3,320 instructions per
  action, the whole gap to Ironclad. Refilling a kept snapshot with `clone_from` measured −16.8% on Silent's W1
  (prototype, not committed: it is in `combat/`, the core lane's); it belongs in stage 1 or as a small core-lane
  item before it.

### Stage 1: hook dispatch by presence unions, no entry list (4 days)
- **What changes** (`combat/hooks.rs`, `combat/engine.rs`, `combat/state.rs`, `commands/power.rs`,
  `commands/piles.rs`, `commands/creature.rs`): the unions of section 2.3 become state fields maintained where
  powers, creatures, relics, potions, afflictions and enchantments change, and where cards enter or leave combat.
  `Engine::dispatch` and `Hooked::overriding` walk the state in listener order instead of filtering `entries`;
  `Bound` is built during the walk (the kind is at hand), so `bind` and its linear power scan go. Listening cards
  are kept as a small set of ids (`listening_cards: SmallVec<[CardId; 4]>`, maintained on enter/leave/afflict/
  enchant), ordered at pass time by (pile rank, position), so decks full of listening cards cost per pass what
  `cards_moved` costs per move today, not more. `ListenerCache`, `Entry`, `fresh`, `cards_moved`, `card_listens`,
  `listeners_changed` and PF11's in-place list maintenance are deleted (about 600 lines of core). The hook union
  tests (`lacks`, `hooked()`) become reads of a precomputed `CombatState::hooks` (OR of the three unions, updated
  with them), and `Hooked` becomes a `&CombatState` wrapper (the study's lever 4).
- **Content files**: none. `#[sts2_macros::hooks]` and the generated `Kind::hooks()` tables are the inputs.
- **Gain**: the hook system is ~740 instructions per action (13%; `hooked()` 150, `bind` 83, `dispatch` self 67,
  `HookSet::contains` 78, `cards_moved` ~100, `contains_listener` 20, snapshot). Expected **−450 (−8%)**, LA −6%,
  and the per-move listener bookkeeping disappears from the pile cluster. No prototype (the change is the
  rewrite itself); the kernel's `snapshot(mask)` is the existence proof at 2,400 total.
- **Parity proof**: for this release the old `listener_entries` walk stays under `cfg(debug_assertions)` and every
  dispatch asserts the new snapshot equals the old filtered list (same oracle shape as today's `ListenerCache::get`;
  the `regress` profile keeps debug assertions, so `regress.sh`, `regress_runs.sh` and the sweep exercise it on
  every content path). Then the full list of section 5.1 plus the lockstep harness against the stage-0 engine.
  Unit test: the union equals the OR of the listeners' `HOOKS` on random fuzzed combats (the `fuzz_combats`
  pattern).
- **Risks**: listener order for powers removed and re-added in one pass (the re-check keeps PX15's semantics:
  a power removed earlier in the pass is still called if its owner is in combat); affliction then enchantment
  order after a card; `hooks_active` after the player's death; a card in no pile (dupes, `RemoveFromCombat`)
  must leave `card_hooks` by count. **Rollback**: revert the merge; nothing else depends on it until stage 4.

### Stage 2: `legal_actions` fused pass and the action bitset (2 days)
- **What changes** (`combat/mod.rs`, `build.rs`): one pass per hand card reading the card once: unplayable from
  local keywords, cost from `energy_cost.base` when the union lacks the cost hooks (as now), `is_playable`/
  `should_play` only for kinds that override them (a bit in a `CardStatic` table emitted by `build.rs` beside
  `hooks()`), targets from one `target_type` read; and a second output form, `legal_mask_into(&mut ActionMask)`
  (the fixed action space of section 7.2) sharing the same loop, so the list and the mask cannot disagree.
- **Content files**: none (`build.rs` reads the model impls as it does for `hooks()`).
- **Gain**: `legal_actions_into` is 624 per action (11%); expected **−270**. The mask writer replaces the
  prototype's 1,160-instruction translation with ~100.
- **Parity proof**: `legal_actions` order unchanged (the harness compares the lists every step); a test derives
  the list from the mask and asserts equality on the sweep's states; the full list.
- **Rollback**: revert. **Effort**: 2 days.

> **Superseded (2026-10-24):** stages 3 and 4 are implemented per docs/specs/stage4-design.md (Fable), which the owner approved. Stage 3 is folded into stage 4's merge A; where the two documents differ, the design doc wins.

### Stage 3: pile semantics of the kernel (2 days)
- **What changes** (`combat/state.rs`, `commands/piles.rs`): draw pile bottom-first with a top-first view;
  `draw` pops; the batch move (`move_to_pile_bottom`) and `add_cards_to_pile` test `card_hooks` once per batch
  (stage 1 gives it); `after_card_moved` and `can_add_to_pile` fold into the batch as PF14 began. Shuffle keeps
  building its list before any move (RNG order).
- **Content files**: the 11 `draw.to_vec()`/`first()`/`last()` reads keep working through the view; `to_vec` call
  sites are left as they are (a view iterator makes them cheaper, not different).
- **Gain**: the pile cluster is ~790 per action with the shuffle (14%); PF14 left `draw` 245, batch 207,
  `move_to_pile` 112, shuffle ~200. Expected **−200**. The kernel's step 2 (per-pile arrays, draw pile top-last)
  was +21% in the kernel; here most of that was already taken by PF14.
- **Parity proof**: full list; `batch_pile_move_matches_one_by_one`; the harness checks pile contents and order
  every step through `canonical_digest` (which hashes piles in game order, so a reversed store with a wrong view
  shows up at once).
- **Rollback**: revert. **Effort**: 2 days.

### Stage 4: the hot/cold state with narrow ids (10 days; the structural stage)
- **What changes**: section 2.1-2.2. `CombatState { hot: Hot, cold: Cold }`; `Hot` holds the regions
  (`creatures`, `powers` per creature, `cards` hot part, five piles, `allies`, `enemies`, `history.this_turn`,
  `relics`, `potions`, `orbs`) with capacities from the survey; `Card` splits into `CardHot` (kind `u16`, upgrade,
  pile, flags, canonical cost, `data` discriminant; ~16 B) and `CardCold` (keywords, cost structs, affliction,
  enchantment, `CardData`, `stale_calculated`, `clone_of`, `deck_slot`) in a cold `Vec` indexed by `CardId`;
  `Card` as a type stays (a `&Card`-like accessor pair) so the run layer's standalone cards (PF11's finding) are
  untouched. `Power` stays 24 B inline. Ids become `u16`. `Monster::rng` stays `Clone`-only (PF11's policy: a
  silent copy of a stream must not happen) and lives in `Hot` by value, so a snapshot copies it by `clone_from`
  of the struct, never by `Copy`. `Combat::clone_from` becomes one memcpy of `Hot` plus cold `clone_from`s (empty
  in most fights). The replay snapshot (`Combat::run`) and `intent_damage`'s scratch copy use the same path.
  Overflow re-lays out the slab (section 2.1); it is a cold function with a counter the bench reports.
- **Content files**: mechanical. `s.card(id).field` keeps compiling for hot fields; cold fields go through
  `s.card_cold(id)` (sed over ~100 sites: `energy_cost`, `keywords`, `affliction`, `enchantment`, `data`,
  `clone_of`), the same pattern as PF11's `power_data` move (~100 sites, mechanical). No behaviour is re-ported.
- **Gain**: on W1, **0 to −300** (allocation 370 → ~150, bounds checks on `u16` indices into fixed regions, no
  `Option<Box>` on the hot path); the FB2 prototype says a fixed array alone is neutral (+0.9%), so the W1 gain
  here is a stretch item, not a commitment. **LA −15 to −20%** (the copy is 6,400 of 31,800 per action; one memcpy
  of ~6 KB is ~150). **RL**: reset and the replay snapshot become one memcpy; the observation becomes a view
  (section 7.1): −2,000 per RL step. Silent/Necrobinder decks and every real deck with prompts gain the snapshot.
- **Parity proof**: full list; the lockstep harness every step (the two engines now differ in layout, which is
  exactly what it checks); the capacity-overflow path forced in a test (`HOT_CAPACITY_TEST=1` sets every capacity
  to 1 so every fight spills from its first card) and run over the strong sweep with byte-identical tables; the
  kernel's `from_core` projection re-pointed at the new accessors (its differential test is a second lockstep).
- **Risks** (section 6): all of them. **Rollback**: revert; stages 5-9 do not depend on it. **Effort**: 10 days,
  in three merges (ids and `CardHot`/`CardCold`; `Hot` regions for creatures, powers and piles; history and the
  spill), each with the full check list.

### Stage 5: integer damage and block folds with a `Decimal` fallback (3 days)
- **What changes** (`combat/hooks.rs` folds, `commands/damage.rs`, `commands/hp.rs`): the folds collect
  modifiers as (additive `i64`, percentage factor, cap) when every listener's contribution is of the proven form
  (`sts2-kernel/src/num.rs`: exact product in `i128`, one truncation), else run the `Decimal` path. `DamageResult`
  and the query structs lose unread fields.
- **Content files**: none; a listener that returns a non-percentage factor simply takes the fallback.
- **Gain**: `Decimal` ~200 and the damage pipeline's struct copies ~500 (`damage_into` 1,047 inclusive).
  Expected **−150 to −250**.
- **Parity proof**: debug and `regress` builds run both folds and assert equality on every fold (so the sweep
  and `regress.sh` cover all 1,300 models); the kernel's 400,000-case table test ported to core; full list.
- **Rollback**: revert. **Effort**: 3 days.

### Stage 6: code footprint and the delivery build (2 days)
- **What changes**: `#[cold]`/`#[inline(never)]` on `Combat::run`'s interrupt arms, `Unsupported` stops,
  `Engine::emit` building the event only when `records_events`, orb/star/potion commands for decks without them,
  `end_combat`, `tear_down_player`; the `Flow` error type stays a `Box`. PGO training set extended with the RL
  bench mode and W1-all; BOLT kept optional (PF7: +1-3% on PGO).
- **Gain**: measured by `ic_cache_fill_l2` and `ex_ret_brn_tkn` per action, not instructions: PGO cut L2 code
  fills 194 → 125 and taken branches 576 → 216 for +37% wall; the study estimates **+5-10% wall on LTO, 2-4% on
  PGO** for the hand-split. No parity risk.
- **Rollback**: revert. **Effort**: 2 days.

### Stage 7: the RL crate (5 days, parallel lane: separate crate, no core edits)
- `crates/sts2-rl`: section 7 (observation, mask, intents cache, `Envs` batch, start corpus). Starts after
  stage 0 on today's state API; after stage 4 the observation's dynamic part becomes a memcpy of `Hot` regions
  without an API change for the trainer.
- **Parity proof**: the observation is not a referee surface, but the mask is derived from `legal_actions` (stage
  2's shared loop) and the intents cache has an oracle (section 7.3). The RL bench digest must not change with
  `--envs` or `--threads`.
- **Done (RL7, 2026-10-24; details and numbers in `docs/perf.md`, "Stage 7 of the RL 2× plan").** `crates/sts2-rl`
  (mask, observation, cost and intent caches checked against the slow paths in debug and `regress` builds, `Envs`,
  `StartRecord` corpus from `cargo xtask sweep --emit-starts`); the RL bench drives `Envs`, digest unchanged. RL step
  24,975 → 18,774 instructions (extraction 1.50×) on the public API; the intent fold and a cached cost accessor
  (core asks, prototyped) take it to 12,915 (2.81×). Until they exist the intent fold is cached with its inputs
  in the key, not run live as section 7.3 describes.

### Stage 8: the C ABI and the PufferLib binding (3 days, parallel lane)
- `crates/sts2-ffi` (the only crate allowing `unsafe`, `#![deny(unsafe_op_in_unsafe_fn)]`, reviewed): section 7.4.
  A Python `PufferEnv` wrapper over ctypes with zero-copy numpy views on the engine-owned buffers.
- **Done (RL8, 2026-10-24; details and numbers in `docs/perf.md`, "Stage 8 of the RL 2× plan").** `crates/sts2-ffi`
  (header `include/sts2.h`, `catch_unwind` at every entry point, a panic poisons the batch, stable buffers, source
  hash as `sts2_build_id()`), `python/sts2/` (`Sts2Envs`, `Sts2PufferEnv`, the stale-build check), append-only id
  vocabularies in the observation, the observation leak test and the `fuzz/` target over the ABI (10 minutes clean).
  Python steps 311k/s at 64 envs and 349k/s at 256 against 484k/471k from Rust through the same ABI.

### Stage 9: the run-level environment (5 days, parallel lane)
- Whole runs as episodes: `RunAction` indexed into a fixed action space beside the combat one, the run's
  observation (floor, gold, HP, deck, relics, map choices, offered rewards, event page), reset = `Run::new(seed)`
  (28 µs, negligible against 500-1,000 actions per run). This is the owner's actual RL target; it needs stages 7-8
  and nothing from the core stages.
- **Done (RL9, 2026-10-24; details and numbers in `docs/perf.md`, "Stage 9 of the RL 2× plan").** `sts2_rl::RunEnvs`
  (`run_envs.rs`) over the run space (`run_action.rs`: the combat space's 140 indices, then 215 run-level ones),
  the run observation (`run_obs.rs`, 2,996 bytes), seeds from a list, a range or a master seed (`seeds.rs`), single
  options taken automatically and counted, and the hybrid mode of `docs/reviews/spire-mentor.md` idea 1
  (`resolve.rs`: the sweep's greedy combat policy, moved here from `sts2-diff`, or a random one, settles every
  fight). Exposed as `sts2_run_envs_*` (ABI 2) and `sts2.Sts2RunEnvs`/`Sts2RunPufferEnv`; `--mode run-env` bench.

## 5. Parity proof machinery

### 5.1 The check list every stage runs
`cargo xtask gate`; bench digests identical under LTO and `--pgo` for fights `ad55be60f5bd8a71`, LA
`85229a762f9361e1` (both copy modes), runs `1febef7e3b528fe5`, 2,000 runs `af6c19933b438172`, every character
`125847415c9a423a`, 32,000 runs on 16 threads `bfb062b545664dda`, plus the new W1-all and RL digests recorded at
stage 0; `tools/regress.sh 5` (6,175 ok) and `tools/regress_runs.sh 8` (497 ok); `cargo xtask sweep --runs 1000
--strong` byte-identical; `tools/replay_history.py --db --sample 300 --jobs 4` byte-identical; `cargo test -p
sts2-kernel --release` (12,160 differential fights, 0 mismatches) with `KERNEL_DIFF_FIGHTS=2000` once per stage;
the capacity-overflow run (stage 4 on); and the lockstep harness below. Instruction counts recorded in
`docs/perf.md` per step as PF11-PF14 did.

### 5.2 The lockstep oracle (new, stage 0)
`crates/sts2-lockstep` (test-only binary, `regress` profile): depends on the current `sts2-core` and on the
previous engine as `sts2-core-prev = { git = "file://$HOME/spirebot", rev = "<stage base>", package =
"sts2-core" }` (two versions of one crate in one binary is allowed; the rev is bumped at each merge). It builds the
same `CombatConfig` on both (every character × every ported encounter × ascensions 0, 10 and the hashed middle
one, plus the loadouts of `tools/regress.sh` and run-derived loadouts from the sweep), drives both with the same
seeded policy (uniform and play-biased halves, as the kernel's test does), and at every step asserts: the legal
action lists are equal and in the same order; `canonical_digest` is equal; statuses and pending prompts are
equal; RNG counters per stream are equal. Events on in half the fights and compared. On a mismatch it prints the
step, both digests and the first differing field. Budget: 50,000 fights per stage in about a minute on 8 jobs.
This is the "run old and new engines in lockstep in debug builds" oracle; it is cheaper than the referee and
catches layout and order bugs the referee's ascension/encounter sampling could miss.

## 6. Risky semantics a fixed-size or `Copy` layout can silently break

| risk | where it bites | how the plan handles it |
|---|---|---|
| **Unbounded piles and cards in combat** (generated cards, clones, statuses) | any hard capacity; `Unsupported` would break the generalist rule | spill by slab re-layout, never refusal; capacities from the survey ×2; `HOT_CAPACITY_TEST=1` forces the spill on every fight through the sweep |
| **Summons beyond fixed creature slots**; ids of dead creatures stay valid (`creatures` is every creature ever created) | summoners over a long fight; `CreatureId` as `u16` | creature region spills like the others; ids are indices into the region and never reused; the Waterfall Giant/Two-Tailed Rat/Living Fog fights are in regress |
| **Removed powers are still read** by models mid-pass (PX15 Constrict; Poison on a dead Giant) | a `Copy` power list that drops removed instances | `removed_powers` keeps instances (inline slots, cold spill); `power(owner, id)` searches both as now |
| **Listener order** (allies then enemies in slot order; powers in application order then the monster; relics, potions by slot, cards Hand/Draw/Discard/Exhaust/Play each followed by affliction then enchantment) | stage 1's walk; stage 3's reversed draw pile; `sort_enemies_by_slot_name` | the walk is written against `for_each_listener`; the old walk is the debug oracle for one release; the draw pile view keeps top-first order for listeners; the harness compares legal-action order every step |
| **Mid-pass membership** (`contains_listener` re-check; a power removed earlier in the pass still runs) | dropping the snapshot or the re-check for speed | both kept; the snapshot is a `SmallVec` as today |
| **RNG call order** (shuffle's list built before the move; `Position::Random` draws; monster `Rng` per creature; `Niche` draws at creature creation) | any reordering of pile work or creature creation in stages 3-4 | RNG counters per stream are in `canonical_digest` and compared every step by the harness |
| **Decimal math** (`(int)` truncation toward zero; 0.75/1.5/0.7 factors; Intangible cap; `WasFullyBlocked` from the exact amount) | stage 5's integer path | integer path only inside the proven domain, fallback otherwise, both computed and compared in debug and `regress` builds |
| **History read back by content** (`this_turn` entries, counts, `last_attacks`, `hp_loss_hits`) | treating history as a log that can be bounded or dropped | stays game state in `Hot`, fixed turn list with spill; history counts in the digest |
| **Events read back** (the TUI, `sts2-diff` traces, `check_replay` compares them) | a `Copy` core cannot own a `Vec<Event>` | events stay in `Cold`/the wrapper; `Snapshot.events_len` semantics unchanged |
| **Replay (model C)**: a step is re-run from its snapshot with the decision log | a snapshot that copies `Hot` but not the cold parts a step mutated (`power_data`, `cost_lists`) | the snapshot copies `Hot` by memcpy and every cold list by `clone_from`; `REPLAY_CHECK` in debug builds re-runs every step and compares |
| **`Monster::rng` copied instead of advanced** (PF11's reason for `Clone`-only) | a `Copy` `Hot` would make it `Copy` | `Hot` is not `Copy`; it is `clone_from`-able by memcpy of plain data with `Rng` kept non-`Copy` by type |
| **Card cold fields read in hot paths** (keywords at turn end, cost modifiers in `legal_actions`) | stage 4's split putting a hot read in `Cold` | the split is driven by the callgrind read counts; `energy_cost.base` and local keywords stay hot |
| **Potion slots and `unported_potions`** (slots are positions, not a list) | compacting slots | slot arrays stay positional |

## 7. The RL interface

### 7.1 Observation
Two engine-owned buffers per env, little-endian plain integers, no floats, no one-hot (featurisation happens in
numpy/torch on the batch, vectorised):
- **Static part** (written at reset only): character, ascension, act, encounter id, room type, the deck as
  (kind `u16`, upgrade `u8`) × up to 64, relics as kind `u16` × 32, potion slots × 6.
- **Dynamic part** (written every step; after stage 4 a memcpy of `Hot` regions, before it a linear pass like the
  FB2 prototype's `observe`): player scalars (hp, max hp, block, energy, max energy, stars, turn, round, pile
  sizes, orb slots, status); creatures × 8: alive, side, slot, monster kind, hp, max hp, block, powers × 12 as
  (kind `u16`, amount `i16`); cards in combat × capacity as (kind `u16`, upgrade `u8`, pile `u8`, cost `i8`,
  flags `u8`) in `CardId` order, which is creation order, so the draw pile's order never leaks (the game hides it;
  the discard pile's order is visible and is given by the pile list); the hand as card ids × 10 in hand order;
  intents × 8 as (kind `u8`, damage `i16`, hits `u8`, cards `u8`); pending choice options × 10 as card ids plus
  min/max/picked. About 1.5 KB per env at the survey's capacities.
- Cost: today ~2,250 instructions (prototype, i32 slots); after stage 4 ~150-300 (memcpy plus intents fold).

### 7.2 Legal-action mask and action space
A fixed index space, `[u64; 3]` bitset, written by `legal_mask_into` (stage 2): play (hand slot 0-9 × target
slot 0-7 where 0 is "no target" and 1-7 are enemy slots in `enemies` order) = 80; potion (slot 0-5 × target) =
48; end turn; pick option 0-9; done; 140 entries. Decoding an index is two divisions and two array reads. The
list form stays for search and the TUI. The run-level space (stage 9) is appended after it.

### 7.3 Intents without the per-step scratch copy
Today `intents()` clones the state and performs the move on a scratch engine per enemy per call (12,700
instructions per RL step with two enemies). Design: an **intent cache** on the env (not in the state): per enemy,
(move token = (state index, forced move, roll counter), base attacks as the scratch run found them). The base
is recomputed only when the token changes (a roll, a forced move, a stun, a revive), on an env-owned scratch
copy refilled by `clone_from` (one memcpy after stage 4). Each observation folds the current modifiers on the
live state (`Hooked::modify_damage` with the player as target and `ValueProp.Move`, exactly `intent_damage`'s
second half) over the cached base: ~100 instructions per enemy, no copy. **Oracle**: debug and `regress` builds
recompute through the scratch path on every observation and assert equality, and the RL bench mode runs in the
sweep's profile over all content once per stage; a move whose base reads state at perform time would fail the
oracle and gets `MonsterModel::intent_is_dynamic()` (default false; such a move recomputes every observation).

### 7.4 Batch API and the C ABI
`sts2_rl::Envs { combats: Vec<Combat>, starts: StartSource, obs: Vec<u8>, masks: Vec<u64>, done: Vec<u8>,
won: Vec<u8>, intents: Vec<IntentCache>, scratch: Combat }` with `new(n, start_source, seed)`, `reset_all()`,
`step(actions: &[u16]) -> StepStats` (steps every env, auto-resets finished ones from the next start and
counts them), `obs() -> &[u8]`, `masks() -> &[u64]`, `done()`, `won()`, `snapshot(i) -> Combat` and
`restore(i, &Combat)` (for search and for reproducing a trajectory), `threads(t)` (envs split across `t` scoped
threads inside `step`, each thread owning its envs; PufferLib's multiprocessing is the alternative and both are
supported). Rewards are not computed by the engine: `done`, `won`, hp and block are in the observation and the
trainer shapes them. The C ABI (`sts2-ffi`): `sts2_envs_new(n, start_source_path, seed, threads) -> *mut`,
`sts2_envs_step(envs, actions_ptr, n) -> i32`, `sts2_envs_obs_ptr/mask_ptr/done_ptr/won_ptr`, `sts2_envs_obs_bytes`,
`sts2_envs_reset_all`, `sts2_envs_free`; buffers are engine-owned and stable, so Python wraps them once as numpy
arrays (zero copy). Errors are return codes; nothing panics across the boundary (`catch_unwind` at the edge).

### 7.5 Reset from a saved snapshot and the start corpus
A start is a `CombatConfig`-shaped record (character, seed string, ascension, act, encounter, deck with upgrades,
relics with data, potions, HP, gold, floor) serialised as JSON lines, generated by `cargo xtask sweep --emit-starts`
from the greedy runs; `Combat::new` on it costs ~450 instructions plus `start_combat`, so a start is built once per
corpus entry and cached as a `Combat` the env resets from with `clone_from`. Reset is therefore one memcpy per
episode after stage 4 and ~1,300 instructions before it. For whole-run RL (stage 9) the start is a seed.

## 8. Budget, effort and order

Instruction budget per W1 action (Ironclad; today 5,541; after PF15 ~5,250):

| item | committed | stretch |
|---|---:|---:|
| PF15 (`may_prompt` cache, dispatch body) | −200 | |
| stage 1 hook walk | −450 | −100 |
| stage 2 legal actions | −270 | |
| stage 3 piles | −200 | −100 |
| stage 4 hot/cold state (allocation, bounds, `Option<Box>`) | −100 | −400 |
| stage 5 integer folds | −150 | −100 |
| stage 6 footprint | −50 (cycles −10%) | |
| **total** | **−1,420 → ~4,100** | **→ ~3,400** |

At IPC 2.8-3.1 (the footprint after stages 1, 4 and 6; PGO already reaches 2.83) that is ~1,350 cycles committed
(~160k LTO fights/s, 1.6×) and ~1,100 stretch (~200k, 2.0×); PGO on top: ~210k committed, ~250k stretch. On the
RL step: 8,950 today → ~4,400 committed (2.0×) and ~3,700 stretch (2.4×), intents cached.

| stage | days | lane | depends on | rollback |
|---|---:|---|---|---|
| 0 oracles, workloads, Silent profile | 4 | tooling + core | | n/a |
| 1 hook walk | 4 | core | 0 | revert merge |
| 2 legal actions + mask | 2 | core | 0 | revert |
| 3 piles | 2 | core | 1 | revert |
| 4 hot/cold state, narrow ids (3 merges) | 10 | core | 1, 3 | revert per merge |
| 5 integer folds | 3 | core | 0 | revert |
| 6 footprint, PGO set | 2 | core | 1 | revert |
| 7 RL crate | 5 | parallel | 0 (gains from 2, 4) | n/a (new crate) |
| 8 C ABI + PufferLib | 3 | parallel | 7 | n/a |
| 9 run-level env | 5 | parallel | 7, 8 | n/a |

Core lane order: 0 → 1 → 2 → 5 → 3 → 4 → 6 (stages 2 and 5 are small and independent, so they land before the
structural stage and give early measured wins; stage 4 is last among the core stages because it is the longest
and its W1 value is the least certain, while its LA/RL value is certain). Parallel lane: 7 starts after 0, then
8, then 9. About 40 worker-days in all; the core lane is serialised (one core engineer), so calendar time is set
by it: about 27 working days.

Rejected alternatives, for the record: a pure `Copy` state with hard capacities and `Unsupported` on overflow (the
kernel's rule; breaks the generalist decision of 2026-10-23); `SmallVec`/enum-spilled lists (PF9: +4-6% W1,
re-confirmed as the inline/heap branch); a generic `Engine<S: Storage>` with two monomorphisations (sound, but
every content impl becomes generic, 1,300 files, and the code size doubles); lockstep/SoA batching of envs (PF6:
loses at every lane count; the RL mode's round-robin is the honest shape and showed identical instruction
counts at 1, 64 and 256 envs).

## 9. Reproduction

```
# FB2, uncommitted prototypes
cargo build --profile fast -p sts2-bench
taskset -c 5 perf stat -e instructions:u -x, target/fast/sts2-bench --mode fights --seeds 1100 --reps 50
taskset -c 5 perf stat -e instructions:u -x, target/fast/sts2-bench --mode rl --seeds 1100 --reps 50 --envs 64 [--no-obs] [--intents]
BENCH_CHAR=Silent taskset -c 5 perf stat -e instructions:u -x, target/fast/sts2-bench --mode fights --seeds 1100 --reps 50
RL_HW=1 target/fast/sts2-bench --mode rl --seeds 2200 --reps 50 --envs 1      # high-water marks
# profiles: target/scratch/{base,pile,Silent,Necrobinder}_w1.cg (callgrind, frame-pointer fast build)
```
