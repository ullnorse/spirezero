# Fixed-point combat kernel prototype (task PF6, perf-plan items 7, 8, 11)

`crates/sts2-kernel` is a second, experimental combat engine. It is a `Copy`, heap-free fight state
with one table-driven interpreter, and it is bit-exact with sts2-core on what it supports. The
question was how fast a spire-mentor-style kernel can be while staying exact, and whether it is
worth adopting.

**Short answer:** with PGO it runs 6.8× faster than sts2-core and 4.9× faster than spire-mentor on
one core (W1). It plays all 110,000 W1 fights with sts2-core's exact digest. It supports 38 of
the 42 Act 1 fights. On a search-shaped workload it is 11× faster than sts2-core. It is a **go as a search/rollout fast path**, and a
**no-go as the engine core** for now (see the recommendation below).

## Design

**Start from sts2-core, then take over.**
- `Fight::from_core(&Kernel, &Combat)` projects an sts2-core combat into the kernel state, or
  returns `Unsupported` with the reason.
- sts2-core does the set-up and plays the first turn start (encounter generation, HP rolls,
  initial rolls, Innate, the first draw). So the kernel never has to port set-up.
- After that, `Fight::legal_actions` and `Fight::step` mirror `Combat::legal_actions` and
  `Combat::apply` in the same order. A policy that picks by index therefore plays the same fight
  on both engines.
- Outside the supported content, `from_core` refuses. A fight that outgrows a capacity mid-way
  stops as `Status::Unsupported`. That has never happened in the tests.

**State** (`state.rs`). `Fight` is 1,960 bytes, `Copy`, with no `Vec`, `Rc` or `dyn`:
- At most 8 creatures with at most 6 powers each. A `Power` is 16 bytes. Each creature has a
  32-bit presence mask of its power kinds.
- At most 120 cards, each 3 bytes (spec index, pile, affliction). Ids are sts2-core's `CardId` /
  `CreatureId`.
- Each pile is a fixed array. The draw pile is stored bottom first, so drawing is a pop.
- The three RNG streams the supported content draws from (`Shuffle`, `MonsterAi`, `Niche`) are
  stored as `RngState`. Each draw goes through sts2-core's own `Rng` (`from_state` / `state`), so
  the f64 draw code is shared, not copied.
- A monster's move log is kept as its last three states plus a 16-bit "ever used" mask. That
  covers every repeat rule and cooldown in scope; `tables.rs` asserts it.
- `canonical()` renumbers power uids and clears stale slots. `digest()` is FNV-1a over the
  canonical state.

**Tables** (`tables.rs`, built once by `Kernel::new` from sts2-core's public API):
- Read from sts2-core's data:
  - card costs, types, targets, keywords and var values per upgrade level;
  - the shuffle sort order (`CardModel.CompareTo`);
  - power types and flags;
  - monster HP ranges and the move state machines (states, follow-ups, branches, weights,
    repeat rules, cooldowns);
  - encounter slots.
- Declared in the kernel: sts2-core's `OnPlay` and `PerformMove` are code, not data. The kernel
  declares them as short op lists (`CardOp`, `MoveOp`: attack, block, apply, add cards, summon),
  about one line per move.
- `Kernel::new` checks the declared lists against sts2-core's move tables: every move id must be
  a move state, and every move state must have a list.
- A real codegen step would need sts2-core's moves to become data (see the recommendation).

**Interpreter** (`engine.rs`, about 1,900 lines):
- One function per sts2-core function, in sts2-core's order of steps, guards and hook passes.
  Hooks that no supported listener overrides are left out.
- Async hook passes snapshot their listeners and re-check each one, like `Engine::dispatch`. A
  presence mask lets a pass with no possible listener cost a few ANDs (perf-plan item 8).
- Irregular behaviours are hand-written: Plow's, Shriek's and Asleep's stuns, Illusion's revive,
  Infested's Wrigglers, Ringing, Tangled, Ravenous, Suck, Thorns, Ritual, Plating, Skittish,
  Hardened Shell, Vigor and Intangible.

**Fixed point** (`num.rs`):
- Damage and block are an integer base plus integer additives (Strength, Vigor, Dexterity), times
  percentage factors (Vulnerable 150, Weak and Frail 75, Shrink 70, Slow 100 + 10n), then the
  Intangible cap. The product is computed exactly in `i128` and then truncated.
- Every later step reads only the truncated value, plus whether the exact amount was positive
  (for `WasFullyBlocked`): block, Hardened Shell, Slippery, Intangible, kill. The proof is in the
  module doc.
- This equals sts2-core's `Decimal` whenever the product's mantissa is below 2^96 at scale 2k ≤ 28.
  sts2-core's `Decimal` panics outside its exact domain anyway.
- Outside the domain the kernel refuses the fight. The unit test compares against `Decimal` on
  every base in -20..2000 times every pair and triple of factors (over 400,000 cases).

**Shuffle.** .NET's `List.Sort` of up to 16 items is an insertion sort, which is stable, except
for exactly 3 items (three `SwapIfGreater` calls, which is not stable). So for n ≤ 16 and n ≠ 3
the pre-shuffle sort is a stable sort by rank. The other sizes use sts2-core's introsort port.

**Lockstep batch** (`batch.rs`, item 11). `run_lockstep` steps N fights one action each,
round-robin. A true structure-of-arrays layout was not built. Every step's control flow diverges
per fight (different hands, monsters and piles), so there is nothing to vectorise. Splitting
fields across N arrays would only scatter each step's reads.

## Supported scope

**Characters and decks.**
- Ironclad and Silent starter decks: Strike, Defend, Bash, Neutralize and Survivor (including
  its discard choice), each unupgraded or upgraded.
- Starter relics: Burning Blood and Ring of the Snake.
- Status and curse cards: Slimed, Infection, Wound, Dazed, Beckon and Ascender's Bane.
- No potions.
- Ascensions 0-10 are covered (Ascender's Bane, tough and deadly enemies).

**Encounters (38 of Act 1's 42).**
- All 22 Overgrowth hallway, elite and boss fights.
- 16 of the 20 Underdocks fights:
  - hallway fights: both Corpse Slug fights, Cultists, Fossil Stalker, Haunted Ship, Punch
    Construct, both Seapunk fights, Sewer Clam, Sludge Spinner and Toadpoles;
  - elites: Phantasmal Gardeners, Skulking Colony and Terror Eel;
  - bosses: Lagavulin Matriarch and Soul Fysh.
- That means 43 monster kinds (Eye With Teeth and Wriggler summons included) and 28 powers.

**Refused.** `from_core` refuses the following, and `unsupported_content_is_refused_cleanly`
checks it:
- The 4 remaining Underdocks fights:
  - Gremlin Merc: stolen gold, a gold reward and an escape. These are outside the fight state
    the kernel carries.
  - Two-Tailed Rats: dynamic branch weights and summons.
  - Living Fog: Gas Bomb summons and Smoggy.
  - Waterfall Giant: Steam Eruption's forced transitions.
- Any other card, relic, potion, enchantment or orb.
- Other characters' decks.

## Differential test

`tests/differential.rs` plays both engines side by side. At every step it checks that:
- the legal-action lists are equal and in the same order;
- the kernel's canonical state equals `from_core` of sts2-core's state, digest included;
- the statuses are equal;
- the 9 run streams the kernel does not carry never move.

The fights it plays:
- Ironclad and Silent on every supported encounter, at ascension `seed % 11`.
- Half the fights use a uniform random policy. The other half use a play-biased policy, which
  makes fights long.
- Every fourth fight starts at 400 HP with randomly upgraded cards, which reaches Ceremonial
  Beast's second phase, deep reshuffles and capacity edges.

The test also asserts that the irregular paths actually ran:
- the Plow stun, the Illusion revive, summons, reshuffles and an Artifact negation;
- afflictions and Slow counting;
- every player debuff, including negative Strength and Dexterity;
- Ritual, Plating, Thorns, Ravenous, Suck, Skittish, Hardened Shell, Shriek, Vigor, Asleep and
  Intangible on enemies.

Results:
- `cargo test -p sts2-kernel --release`: 12,160 fights, 344,348 compared steps, 9,395 Survivor
  choices, **0 mismatches**.
- `KERNEL_DIFF_FIGHTS=2000`: 152,000 fights, 4,307,583 steps, 0 mismatches (30 s).
- Debug builds (the gate) run 6 fights per pair.
- Mutation check: changing Vulnerable to 1.6, removing Frail or Weak, Territorial's amount, the
  Ringing veto, Ethereal exhaust, the Wriggler slots, the shuffle order, or Shrink's infinite
  case each fails within the first few fights.

The bench is a second, whole-workload check. `sts2-bench --engine kernel` reproduces W1's
sts2-core identity exactly over all 110,000 fights: digest `ad55be60f5bd8a71`, 35,868 wins,
2,412,377 actions.

## Throughput (W1: 2200 seeds × 50 fights)

Machine: Ryzen 7 7800X3D, WSL2, shared with other workers (load 3-4). One-core runs are pinned
to `taskset -c 13`, because another worker was benching on CPU 9. Rounds are interleaved across
all builds, and the figures are clean-round ranges. A few runs disturbed by neighbours read
30-50% low and were discarded.

- `fast` = `[profile.fast]` (fat LTO, `codegen-units = 1`).
- PGO = `cargo xtask bench --pgo`. Its training now also plays the kernel engine.
- spire-mentor is `8e0e917` `site_random_bench` (fat LTO, no PGO), run on the same core between
  our runs.

| W1 | 1 core, fights/s | ns/action | 16 threads, fights/s |
|---|---:|---:|---:|
| sts2-core, LTO | 37,900-39,000 | 1,170-1,200 | 345,000-363,000 |
| sts2-core, PGO | 51,300-53,400 | 855-890 | 480,000-497,000 |
| spire-mentor, LTO | 70,200-74,200 | ~630 | 702,000-721,000 (16 pinned processes) |
| **kernel, LTO** | **293,000-294,000** | **155** | **2,770,000-2,840,000** |
| **kernel, PGO** | **356,000-362,000** | **125-128** | **3,450,000-3,520,000** |
| kernel lockstep, 16 lanes, LTO | 227,000-228,000 | 200 | |
| kernel lockstep, 16 lanes, PGO | 231,000-271,000 | 168-197 | |

The table is the final 38-encounter build. The 33-encounter build measured 310-322k (LTO) and
342-366k (PGO) fights/s: the extra hook sites (after-attack, before-damage, the extra turn
passes) cost about 8% under LTO and nothing measurable under PGO.

Notes on the table:
- 16-thread rates are the sum of the per-thread rates (PF1's convention).
- Speedups, one core: kernel PGO is 6.8× sts2-core PGO and 4.9× spire-mentor. LTO against LTO,
  the kernel is 4.1× spire-mentor. All cores: 7.1× sts2-core PGO and 4.9× spire-mentor.
- Instructions per action (plain `--release`, timed part only, `perf stat -e instructions:u`):
  sts2-core 14,852, kernel 2,381. spire-mentor measured 6,600 in PF3.

**Lockstep.** On the 33-encounter build, lanes 1, 2, 4, 8, 16 and 64 (PGO) ran at 366k, 305k, 308k, 289k, 276k and 240k
fights/s. One lane equals the plain loop, and every extra lane costs throughput. Interleaving
fights disturbs the branch predictor and the L1 working set (16 lanes × 2 KB fill the L1 by
themselves). There is no memory-level parallelism to win: a fight's working set already lives
in L1. **Item 11 is a no-go.**

**Search-shaped workload** (`--lookahead`: before each action, copy the fight and apply every
legal action to the copy; same digest). LTO, one core:

| | fights/s | ns/action |
|---|---:|---:|
| sts2-core | 5,090 | 8,950 |
| kernel | 56,900 | 801 |

The kernel is 11× faster here (14× on the 33-encounter build: 4,460 against 62,900). A copy is one 2 KB `memcpy`, against sts2-core's `Combat::clone`
with its `Vec`s.

**Where the kernel's time went, and the changes that helped** (1 core, release; digest
unchanged at every step):

| Step | fights/s | Change |
|---|---:|---|
| first working version | 169,000 | |
| step 1 | 190,000 | hook snapshots skipped when no listener can match |
| step 2 | 230,000 | per-pile arrays, draw pile top-last (pile moves were 35% of the time, mostly `memmove`) |
| step 3 | 250,000 | `legal_actions` works out guards, Ringing, Tangled and targets once per call, not per card |
| step 4 | ~290,000 | caller-reused action buffer (`legal_actions_into`; initialising the 64-slot buffer cost 7.5%), bucket pre-sort for shuffles |

What remains is spread over the turn flow:
- start of turn 37% (draw 15%, shuffle 14%, of which the RNG is 8%);
- `legal_actions` 17%;
- enemy turn 12%;
- card play 12%.

## Recommendation

**Go, as a fast path for search and training.**
- On its scope it is exact (proven step by step, and over whole workloads by the W1 digest).
- It is 6-7× faster than sts2-core on rollouts and 11-14× faster on clone-heavy search.
- It drops in cleanly: try `Fight::from_core`, use the kernel when it succeeds, and fall back to
  sts2-core on `Unsupported`. Because legal actions are listed in the same order, a search can
  mix the two engines.
- Keep the differential test in the gate (as now) so that any sts2-core content change touching
  supported content shows up at once.

**No-go, for now, as the engine core.**
- Its scope is a few percent of the content (starter decks, 38 encounters, 28 powers). Every
  card, power or monster would be ported twice.
- Its speed comes from fixed capacities and from leaving out every hook nothing in scope
  overrides. Both shrink as scope grows.
- It relies on sts2-core for combat set-up and the first turn.

Making it the core would mean rebuilding sts2-core around the same ideas, which is a design
decision for the user.

**What transfers to sts2-core** (independent of adopting the kernel), in rough order of payoff:
1. Make monster moves and simple card effects data (`MoveOp`/`CardOp`-style lists next to the move
   tables). This also makes the kernel's tables real codegen instead of declarations.
2. Per-creature power-presence masks, to skip hook passes with no listener (item 8).
3. Pile storage with the draw pile top-last, plus a reusable `legal_actions` buffer.
4. Integer fixed-point for damage and block (the `num.rs` argument carries over).
5. A `Copy` snapshot form of the state for search.

**Widening the kernel next.**
- Act 1's last 4 fights: Smoggy and Gas Bombs, Steam Eruption, dynamic weights, and (for Gremlin
  Merc) carrying gold and extra rewards in the state.
- Then the starter decks' common reward cards, which matter most for search.
- Each is a power or two plus a hook site, checked by the same differential test.

Credit: no spire-mentor code is copied. The design ideas (presence masks, `Copy` state, fixed
capacities) follow its engine as surveyed in `docs/specs/perf-plan.md`.
