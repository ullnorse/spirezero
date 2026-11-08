# Stage 4 design: the hot/cold combat state with narrow ids (2026-10-24)

Decision document for stage 4 of `docs/specs/rl-2x-plan.md` (sections 2.1-2.2, 4 "Stage 4", 5, 6, 7). The
implementing agent follows this; where it differs from the plan, section 8 says so and why. Written from the
worktree `SD4` at `f171aa5` (`main` plus RL0, RL7, PF15, CQ1 pending). Nothing in `crates/*/src` is changed by
this document; the measurements in section 9 came from uncommitted scratch (a size-printing test, the capacity
survey rerun) and were reverted.

Inputs: the plan; `docs/perf.md` (RL0: the capacity survey, the Silent snapshot finding; RL7: the RL crate and
its core asks; PF9, PF11-PF14); `docs/reviews/perf-architecture.md` (W1 is front-end bound; `clone_from` is 19%
of a lookahead action; fixed arrays alone are neutral on W1); `docs/reviews/spire-mentor.md` (fuzz target,
observation leak test); `docs/specs/copy-state-plan.md`; `crates/sts2-core/src/combat/{state,hooks,engine,
history,mod}.rs`, `crates/sts2-core/src/capacity.rs`, `crates/sts2-rl/src/{obs,envs,intents,costs}.rs`,
`crates/sts2-kernel/src/state.rs`; the community run DB (`~/.cache/spirezero-data/sts2fun/sts2_public.db`).

## 0. Decisions in one screen

1. **`CombatState { hot: Hot, cold: Cold }`.** `Hot` is a fixed-shape struct: scalars, the 12 run RNG streams,
   the hook unions, and six *typed slabs* (`Slab<CreatureHot>`, `Slab<CardHot>`, `Slab<u16>` for every id list,
   `Slab<Power>`, `Slab<RelicHot>`, `Slab<Orb>`), each a `Vec<T>` of fixed length holding named *regions*
   `{off, len, cap}`. `Cold` holds `Vec`s copied in proportion to their length: `CardCold` per card, the cost
   lists (flattened), power and relic instance data, the deck, the turn's history, the rare lists. About
   9.5 KB of `Hot` at the capacities below; `Hot` is `Clone` (one memcpy per slab), never `Copy`.
2. **Ids:** `CardId(u16)`, `CreatureId(u16)`, `RelicId(u16)`; pile entries `u16`. **`PowerId` stays `u32`**
   (section 8.5). `PotionId`, `OrbId`, `AttackId` stay `u32` (unique counters, not indices).
3. **Overflow = spill by region re-layout**, never refusal and never a per-access branch: a `push` into a full
   region calls a `#[cold]` `spill` that doubles that region's capacity inside its slab (one `Vec::resize` and
   one memmove of the tail). Contents, ids and every read are unchanged; equality, hashing and the canonical
   digest ignore offsets, capacities and slack, so a spilled state is indistinguishable from an unspilled one.
   The one typed stop: a 65,536th card or creature in one fight ends it as `Unsupported` (section 2.4).
4. **Capacities** come from the spill rate over the normal distribution (the greedy sweep, G, rerun today), not
   the stress sweep's maxima: cards 128, draw 64, discard 64, exhaust 32, hand 10, play 8, creatures 16,
   enemies 8, allies 4, pets 4, powers 8 per creature in a 64-entry slab, removed powers 32, relics 32,
   potion slots 8, orbs 10 (section 2.2). Every one spills in under 0.1% of normal states and in 0% of W1.
5. **Snapshot, reset and restore are `clone_from`**: one struct copy for the `Hot` header, one memcpy per slab
   (6), one memcpy per non-empty cold list (~10, most empty in most fights), no allocation in steady state
   (every buffer is retained across `clone_from`; a fresh `clone` allocates once). The "one memcpy" ideal holds
   for the RL readout and for each slab; the full state is a bounded sequence of memcpys with no per-field
   logic, which is what the lookahead and the replay snapshot pay for today (section 6).
6. **Three merges** (section 5): **A** ids, the `Slab` type, every id list as a region, piles with the draw pile
   bottom-first (this absorbs stage 3); **B** `CardHot`/`CardCold`, creatures and powers as slabs, `Power` to
   20 B; **C** the retained replay snapshot and scratch copies through `clone_from`, cold lists flattened,
   `Envs` reset and the observation on the new regions, the fuzz target. Each merge carries the full gate plus
   the forced-spill runs; rollback is the merge commit.
7. **Expected speed** (section 6): W1 Ironclad −250 to −500 instructions per action over the three merges
   (5,061 → ~4,600-4,800; stage 3's −200 is inside A), W1 Silent −2,900 (the snapshot), LA −10 to −15%, RL step
   −1,000 to −1,500 and the reset 1,300 → ~300. The plan's LA −15-20% is the optimistic end.

## 1. Layout

### 1.1 The split rule

`Hot` holds what the step loop reads per action and what the observation exports; it has a fixed shape so a
copy is a memcpy and an observation is a slice. `Cold` holds payload that is read by one model at a time (a
card's `CardData`, a power's instance data), run-level data the fight only carries (the deck), and lists that
grow with the fight but are never read by offset (the turn's history). Everything in both is game state and is
copied by the snapshot; the difference is only in how.

### 1.2 `Hot`

```rust
pub struct Hot {
    // Scalars (as today, narrowed where an id is stored): rng streams [Rng; 12] (480 B; the seed string moves
    // to Cold), ascension, encounter, room_type, act_index, map_coord, round, side, in_progress, starting,
    // pending_loss, result, next_power_id, next_potion_id, next_attack_id, next_orb_id, living_primary_enemies,
    // prompting, encounter_data, gold_proportion; the Player scalars (character, creature, energy, max_energy,
    // gold, turn_number, extra_turn, phase, orb_slots, stars, ready_to_end_turn, effect_depth, hooks_active).
    // Stage 1's unions: hooks: HookSet (the OR), item_hooks: HookSet, card_hooks: [u16; Hook::COUNT] (counts).
    pub creatures: Slab<CreatureHot>,   // one region; CreatureId = index
    pub cards: Slab<CardHot>,           // one region; CardId = index
    pub ids: Slab<u16>,                 // regions: hand, draw, discard, exhaust, play, allies, enemies, pets,
                                        //          escaped, removed_from_deck (deck indices)
    pub powers: Slab<Power>,            // one region per creature (application order) + `removed` (removal order,
                                        //          owner-tagged)
    pub relics: Slab<RelicHot>,         // one region; RelicId = index. RelicHot { kind: RelicKind, melted: bool }
    pub potions: [Option<Potion>; 8], unported_potions: [Option<&'static str>; 8],  // positional slots (risk 13)
    pub orbs: Slab<Orb>,                // one region, front first; Orb.value as PackedDecimal (24 B)
}
```

**`Slab<T>`** (`combat/slab.rs`, ~150 lines): `data: Vec<T>` whose length is the sum of its regions'
capacities, `regions: [Region; N]` indexed by a per-slab `#[repr(u8)]` enum (or, for the per-creature power
regions, a `Region` stored in `CreatureHot`), `Region { off: u16, len: u16, cap: u16 }`. API: `get(r) -> &[T]`,
`get_mut(r) -> &mut [T]`, `push(r, T)`, `pop(r)`, `insert(r, i, T)`, `remove(r, i)`, `truncate`, `retain`,
`extend_from_slice`, `clear(r)`, `alloc_region(cap) -> Region` (for a new creature's powers). A push into a full
region calls `spill(r)` (`#[cold] #[inline(never)]`, section 2.3). No raw offsets leave the type; a `&[T]` from
`get` is a plain slice bounded by `len`, so iteration is branch-free and reads cost what a `Vec` read costs
(the bounds check against `data.len()` stays; a `u16` index into a region of known `cap` lets LLVM drop it in the
prefix-slice case as the FB2 prototype saw). `Clone::clone_from` is `data.clone_from_slice` when the lengths
match (one memcpy; `Rng` and `Monster` are `Clone`, not `Copy`, and LLVM lowers a derived `Clone` of plain
fields to a memcpy; merge B's callgrind diff confirms the call count) plus a copy of the region table.
`PartialEq`, `Hash` and `Debug` are implemented over `regions.map(get)`: slack and offsets are invisible.

**`CreatureHot`** (~232 B, 16 of them = 3.7 KB): `side: Side`, `hp`, `max_hp`, `block: i32`, `powers: RegionId`
(a `u8` index into the power slab's region list; the slab owns the `Region`, so a spill updates one place),
`power_kinds: PowerKinds` (32 B), `hooks: HookSet` (stage 1's per-creature union, 16 B), `pet_owner:
Option<CreatureId>`, `slot: Option<&'static str>`, `infinite_hp_display: bool`, `monster: Option<Monster>`
(128 B: `MonsterKind` 16, `MoveMachine` 64, `Rng` 40, flags). `removed_powers` is no longer per creature
(below). The `Monster` is the largest item in `Hot` (2 KB of the 3.7 KB); shrinking it (`MonsterKind` as a
`u16` ordinal plus the per-combat fields in `Cold`, a packed `MoveLog`) is the lever if `Hot` must lose 1-2 KB
later; not part of stage 4.

**`CardHot`** (16 B, `#[repr(C)]`, all-integer, `bytemuck::NoUninit`; 128 of them = 2 KB):

| field | type | why hot |
|---|---|---|
| `kind` | `CardKind` (`#[repr(u16)]`) | every read |
| `upgrade_level` | `u8` | `legal_actions`, display |
| `pile` | `u8` (`PileCode::NONE = 255`) | `pile_of`, every move |
| `flags` | `u8` | `exhaust_on_next_play`, `single_turn_retain`, `single_turn_sly`, `is_dupe`, `has_affliction`, `has_enchantment`, `listens` (kind, affliction or enchantment overrides a hook) |
| `keywords` | `Keywords` (`u8`) | `legal_actions` (Unplayable), turn end (Retain, Ethereal) |
| `cost_base` | `i16` | `legal_actions` when the union lacks the cost hooks |
| `cost_canonical` | `i16` | cost display, `CostsX` as a flag bit |
| `star_base` | `i16` | Regent `legal_actions` |
| `base_replay_count` | `u8` | card play |
| `data_tag` | `u8` | `CardData` discriminant, so a model asks whether it has data without a cold read |
| `_pad` | `u16` | explicit padding (`NoUninit` forbids implicit padding) |

**`CardCold`** (~104 B, `cold.cards: Vec<CardCold>` indexed by `CardId`): `energy_cost` extras (`captured_x`,
`upgrade_cap`, `costs_x`), `star_cost` extras (`last_spent`, `upgrade_cap`, `canonical`, `costs_x`),
`affliction: Option<Affliction>`, `enchantment: Option<Enchantment>`, `data: CardData` (36 B), `clone_of:
Option<CardId>`, `deck_slot: Option<u16>`, `stale_calculated: Option<PackedDecimal>`. Content reads these
through `s.card_cold(id)`; the 389 content sites that read `.energy_cost`, `.keywords`, `.affliction`,
`.enchantment` or `.star_cost` are the mechanical sed of the plan (`keywords` and `energy_cost.base` stay on the
hot view, so most of the 79 `energy_cost` sites and all 51 `keywords` sites change only in spelling). The
run layer's standalone `Card` (PF11) is untouched: `Card` stays as the struct content constructs
(`Card::new`, `from_deck`) and `CombatState::push_card` splits it.

**`Power`** (20 B, `NoUninit`): `id: PowerId (u32)`, `amount: i32`, `amount_on_turn_start: i32`, `owner:
CreatureId`, `applier: CreatureId` (`CreatureId::NONE = u16::MAX` for none, so the struct has no `Option`),
`kind: PowerKind (#[repr(u8)])`, `flags: u8` (`skip_next_duration_tick`). A creature's powers are its region
in `hot.powers` (cap 8 at creation, from a 64-entry slab); **removed powers are one owner-tagged region**
(`removed`, cap 32, removal order). `power_instance(owner, id)` searches the owner's region then `removed`
filtered by owner, which is today's `powers.chain(removed_powers)` in a different storage; the canonical
digest hashes `removed` per creature in removal order, which the filter gives exactly.

### 1.3 `Cold`

| list | element | copied by | note |
|---|---|---|---|
| `cards: Vec<CardCold>` | 104 B | one memcpy of `len` | the largest cold copy: ~40 cards → 4 KB |
| `energy_mods: Vec<(CardId, CostModifier)>`, `star_temps: Vec<(CardId, TemporaryCost)>` | | one memcpy each | `CostLists` flattened (today a `Vec` of `Vec`s: a `clone_from` allocates when an entry appears); per-card order is insertion order filtered by id |
| `power_data: PowerDataTable` | 152 B | one memcpy | as today |
| `relic_data: RelicDataTable` | 204 B | one memcpy | new, sparse like `PowerDataTable` (`RelicData` is 200 B and `None` for most relics) |
| `deck: Vec<DeckCard>` | 20 B | one memcpy | mutated in combat (`TheScythe`), so copied; a 6,588-card deck (section 2.1) costs 130 KB per snapshot, correct and rare |
| `history: CombatHistory` | | its own `clone_from` | `this_turn` and `hp_loss_hits` stay `Vec`s (section 8.1) |
| `extra_rewards`, `spawned_enemies` | | one memcpy each | rare, tiny |
| `seed_string: String` | | `clone_from` (no realloc) | moved out of `RunRngSet`'s copied part |
| `spills: u32` | | not compared | the spill counter the bench reports (section 2.3) |

`Combat` keeps `events`, `pending: Option<Box<Pending>>` (request, step, decisions; no state), `unsupported`,
`listeners` (deleted by stage 1) and gains `snapshot: Option<Box<CombatState>>`: allocated the first time a step
needs one, refilled by `clone_from` before each snapshotted step, read back by `state.clone_from(&snapshot)` on
a decision (a copy, not a move: nothing large moves through the stack on any step, which is what cost RL0's
prototype +2.3% on non-prompting decks).

### 1.4 `Copy`, boxes and envs

`Hot` is not `Copy` (PF11's `Rng` policy stands: a stream is copied only by an explicit `clone_from`). A
`CombatState` is one inline header of about 1.2 KB (scalars, unions, six `Vec` headers and region tables) plus
six slab blocks and about ten cold blocks on the heap, allocated by `Combat::new` or the first `clone` and
retained for the life of the `Combat`. `Envs` keeps `Vec<Combat>` inline as today; a reset is `clone_from` into
the retained buffers. A single-block layout (one byte arena with typed views) would need `unsafe` or
`bytemuck::cast_slice` over every list and buys nothing the step loop can feel: what matters, no allocation per
step, bounded memcpys and fixed observation offsets, this layout gives.

### 1.5 The observation after stage 4 (plan section 7.1)

The dynamic part becomes: header scalars (computed, 24 B); creatures × 16 as 12 B each (a loop over `len`
creatures reading `CreatureHot`, ~100 instructions); **the powers slab as raw bytes** (64 × 20 B,
`bytemuck::cast_slice`, one memcpy); **the cards region as raw `CardHot` bytes** (128 × 16 B, one memcpy); the
hand region as raw `u16`s (10 × 2 B); the hand's modified costs (≤10 calls of RL7's cached cost accessor);
intents (RL7's fold over cached bases); the pending choice. About 3.5 KB per step, ~600-800 instructions of
extraction against RL7's 6,685 with the core asks. This needs the trainer to take the engine's records
(16 B cards, 20 B powers) instead of the plan's 6 B card and 4 B power records; a loop writing the compact
records costs ~1,300 instructions per step instead and stays correct, so it is the RL lane's choice
(section 8.6). Leak check (RL8): the cards region is in `CardId` order, the powers region in application order,
the hand region in hand order; the draw region is never exported, so nothing hidden can appear.

## 2. Capacity, overflow and the real maxima

### 2.1 What the content and the community actually do

**Capacity survey, rerun today on `f171aa5`** (`target/regress/sts2-diff sweep --capacities`; G = 200 greedy
runs per character at normal HP, 202,110 states; S = 1,000 `--strong` runs per character, 3,208,987 states; the
RL0 columns R, SG and H in `docs/perf.md` are unchanged). ppm = states over the threshold per million states.

| list | G max | G ppm >8 / >16 / >32 / >64 / >128 | S max | S ppm >32 / >64 / >128 / >256 | first reached (G) |
|---|---:|---|---:|---|---|
| cards in combat | 208 | 1,000,000 / 638,474 / 35,327 / 1,865 / 568 | 2,593 | 569,447 / 85,840 / 5,695 / 2,061 | Waterfall Giant, round 62 |
| creatures ever | 14 | 727 / 0 / 0 / 0 / 0 | 21 | 0 | Ovicopter |
| enemies / allies / pets | 6 / 2 / 1 | 0 | 6 / 3 / 2 | 0 | |
| powers on one creature | 9 | 9 / 0 | 20 | 0 (>16: 23) | Kaiser Crab |
| powers, all creatures | 23 | 10,662 / 192 / 0 | 32 | 0 | |
| removed powers, one creature | 29 | 9,390 / 371 / 0 | 184 | 2,704 / 479 / 92 / 0 | Hunter Killer, round 16 |
| removed powers, all | 32 | 43,253 / 3,255 / 0 | 227 | 6,880 / 1,674 / 248 / 0 | |
| hand / play | 10 / 3 | 5,125 / 0 | 10 / 9 | 0 | |
| draw | 185 | 432,106 / 72,490 / 1,979 / 445 / 118 | 1,749 | 154,287 / 12,059 / 1,860 / 970 | Waterfall Giant, round 59 |
| discard | 190 | 283,622 / 41,244 / 1,726 / 806 / 202 | 1,752 | 50,309 / 5,412 / 1,918 / 1,239 | |
| exhaust | 25 | 6,362 / 440 / 0 | 223 | 3,779 / 933 / 136 / 0 | |
| history entries this turn | 107 | 748,285 / 373,202 / 26,643 / 232 / 0 | 1,114 | 113,552 / 4,932 / 88 / 21 | Waterfall Giant, round 1 |
| relics | 12 | 4,378 / 0 | 59 | 1,310 / 0 | |
| potion slots / orbs / orb slots | 5 / 8 / 8 | 0 | 9 / 10 / 10 | 0 (>8: 2,788 / 86 / 461) | |
| run deck | 43 | 1,000,000 / 581,480 / 10,761 / 0 | 219 | 494,289 / 27,590 / 728 / 0 | |
| cards with cost lists / one card's lists | 30 / 13 | 1,469 / 727 / 0 ; 613 / 0 | 120 / 154 | | |
| power data entries | 10 | 44 / 0 | 59 | 47 / 0 | |
| pending choice options | 29 | 1,840 / 291 / 0 | 816 | 1,532 / 106 / 4 | Knowledge Demon |

**Community run DB** (`sts2_public.db`, 378,079 valid single-player unmodified runs over every public build;
8,126 at v0.111; run-level fields only, so it sees decks, relics, potions and turns, not powers, statuses or
history):

| quantity | p50 | p99 | p99.9 | p99.99 | max | runs over 64 / 128 |
|---|---:|---:|---:|---:|---:|---|
| deck at run end (all builds) | 24 | 53 | 104 | 534 | **6,588** | 2,003 (0.53%) / 309 (0.08%) |
| deck at run end (v0.111) | | | | | **283** | 75 / 10 of 8,126 |
| relics (all builds; v0.111) | 9 | 25 | 30 | 36 | 219 (an old build); **37** at v0.111 | 5 / 2 |
| potion slots held (all; v0.111) | 0 | 3 | 5 | 6 | 18 (old builds); **6** at v0.111 | 0 |
| turns in one fight (v0.109-0.111, 514,772 fights) | | 12 | 16 | 22 | **339** (Terror Eel elite, Necrobinder) | 88 fights over 20 turns |
| floors | 31 | 50 | 51 | 51 | 79 | |

The giant decks are real play, not corruption: the 6,588-card Ironclad deck is 6,565 Perfected Strikes with
Pael's Growth; the next ones (2,940 Crescent Spears, 2,263 Squeezes, 2,242, 2,229...) all carry Pael's Growth.
A fight with that deck starts with 6,588 cards in combat and a 6,500-card draw pile. **Any hard capacity
refuses real runs; the spill is not optional.** The DB confirms the survey's run-level marks (deck > 128 in
0.08% of runs, relics ≤ 37, potion slots ≤ 6 on the current build) and adds the fight-length tail: 339 turns
in one fight is within what the strong sweep already exercises (round 534 and 1,264).

### 2.2 Capacities

Rule: the smallest power of two (or the game's own bound) whose spill rate over G is at most about 0.1% of
states, so the spill path runs in normal play only in the long boss loops, and never in W1 (the bench prints
the spill count; W1 and W1-all must report 0).

| region | cap | G ppm over | bytes | note |
|---|---:|---:|---:|---|
| cards | 128 | 568 | 2,048 | the plan's survey rule (×2 of 208 → 512) would copy 8 KB of mostly slack |
| hand / play | 10 / 8 | 0 / 0 | 36 | the game bounds the hand at 10 |
| draw / discard | 64 / 64 | 445 / 806 | 256 | a shuffle moves discard into draw: bounded by draw+discard, both regions spill independently |
| exhaust | 32 | 0 | 64 | |
| creatures | 16 | 0 | 3,712 | ids never reused (risk 2); 16 × ~232 B |
| enemies / allies / pets / escaped / removed_from_deck | 8 / 4 / 4 / 8 / 8 | 0 | 64 | |
| powers slab | 64 entries, 8 per creature at creation | 9 (per creature >8) | 1,280 | the slab spills when the 9th creature is created (G: 727 ppm) or a creature's region fills |
| removed powers | 32 | 0 (S 6,880) | 640 | |
| relics | 32 | 0 (S 1,310; DB max 37) | 128 | `RelicHot` 4 B |
| potion slots / unported | 8 / 8 | 0 (S 2,788) | 160 | positional arrays with a `Vec` spill in `Cold` only if a 9th slot is ever seen: the game's belt is 5 + Potion Belt; 9 appeared only with 9,999 HP strong loadouts |
| orbs | 10 | 0 | 240 | the game's `OrbQueue.Capacity` max seen |
| header (scalars, 12 streams, unions, region tables) | | | ~950 | |
| **`Hot` total** | | | **~9.4 KB** | above the plan's 4-8 KB; see 8.4 |

Not in `Hot`: history this turn (G: 2.7% of states over 32, 0.02% over 64; 64 B entries: a 128-entry list is
8 KB of mostly slack; section 8.1), the deck (unbounded, run-level), cost lists, power and relic data, pending
options (in `Pending`).

### 2.3 The spill, and why it cannot change a result

`Slab::push(r, x)`: `if self.regions[r].len == cap { self.spill(r) }` then write. `spill` (cold, never
inlined): `new_cap = cap * 2` (or `max(2*cap, len+1)` for `extend_from_slice`), `data.resize(data.len() + (new_cap
- cap), T::default())`, memmove the tail `[off + cap ..]` right by `new_cap - cap`, add `new_cap - cap` to every
region's `off` beyond `r`, set `cap`; bump `cold.spills`. For per-creature power regions the same, with the
creatures' stored `Region`s updated through the slab's region list (the slab owns the regions; `CreatureHot`
holds an index into it, not a copy).

Why it is invisible:
- **Contents are invariant.** A region is a sequence; spill changes where it lives, not what it holds, and ids
  index *within* a region (`CardId` = index into `cards`, so `off` moves and the id does not).
- **No reference survives a mutation.** Every read goes through `get(r) -> &[T]`; a push needs `&mut Slab`, so
  the borrow checker forbids holding a slice across it. There is no raw offset type outside `slab.rs`.
- **Equality, `Hash`, `Debug` and `canonical_digest` see `get(r)` only**, so a spilled and an unspilled state
  with the same contents are equal, hash equal and digest equal. `REPLAY_CHECK` (debug builds re-run every step
  from the snapshot and assert `state == direct`) therefore also covers "the snapshot spilled, the direct run
  did not" and vice versa.
- **RNG order cannot move**: a spill draws nothing and calls no hook.
- **Proof by forcing it.** `STS2_HOT_CAPS=min` (read once at `Combat::new`; a `Caps` table with `DEFAULT` and
  `MIN` = every capacity 1) makes every list spill from its first push. Gates: (a) `cargo xtask lockstep` with
  the working tree at `min` against the pinned base (50,000 fights, 100 runs: 0 differences); (b) the strong
  sweep at `min`, byte-identical tables; (c) `tools/regress.sh 5` at `min` against the referee; (d) a unit test
  in `combat/tests` that drives the same `CombatConfig` and action list at `DEFAULT` and `min` and asserts
  `state == state` after every action over `fuzz_combats`' random combats; (e) the fuzz target (section 4,
  risk 1) checks after every step that regions are disjoint, inside `data`, `len <= cap`, every card's `pile`
  byte names the one region that holds it exactly once, and `card_hooks` equals a recount.

### 2.4 The one typed stop

`CardId` and `CreatureId` are `u16`, so a fight that creates its 65,536th card or creature cannot continue:
`create_card` / `create_creature` return `Interrupt::Unsupported { reason: "card id space exhausted" }` and the
fight ends as `Status::Unsupported`, which every tool already counts and never mistakes for a result. The
largest real deck is 6,588 cards; the strong sweep's 1,264-round fights reach 2,593 cards; the RL episode cap is
10,000 actions. This is the same class as today's `Unsupported` for unported content: loud, counted, never
wrong. Keeping `u32` ids would keep the pile entries at 4 B and the `CardHot` at 20 B for a case no run has
reached by a factor of ten.

## 3. Interactions

**Stage 1 (hook unions, no entry list) before stage 4, as planned.** Stage 1 adds `Creature::hooks`,
`Player::item_hooks`, `CombatState::card_hooks` (counts per hook) and the OR `hooks`; stage 4 moves them into
`Hot` unchanged (`CreatureHot::hooks`, header fields). Stage 1's pass walks `allies` then `enemies`, each
creature's powers in order, the monster or the player's relics, potions and piles: with stage 4 these are
`ids.get(Allies)`, `powers.get(c.powers)`, `relics.get(All)`, `ids.get(pile)`; the walk's code does not change
shape. Stage 1's `listening_cards: SmallVec<[CardId; 4]>` becomes an `ids` region of cap 16 if stage 1 keeps it;
this document recommends stage 1 drop it and skip the card walk by `card_hooks` alone, then walk the piles
testing `CardHot::flags & LISTENS` (one byte per card, in pile order, only on passes some card listens): one
less maintained list, and after merge B the walk touches 16 B per card. Stage 4 supports either. Stage 4 after
stage 1 also means the pile batch moves no longer call `cards_moved`, so merge A's pile code has one job.

**Stage 2 (fused `legal_actions`, `CardStatic`)** reads `kind`, `keywords`, `cost_base`, `star_base` and
`pile`: all `CardHot`. Mechanical after merge B.

**Stage 3 (piles bottom-first, batch moves) is absorbed into merge A** (section 8.3): the draw region stores
the top card last, `draw` pops, `Player::pile(Draw)` returns a `PileView` whose `iter()`, `first()`, `last()`,
`len()`, `contains()` and `to_vec()` are top-first (the 65 content reads of `player.draw`/`hand`/`discard`
that use those keep compiling); `canonical_digest` hashes through the view, so a wrong view fails the lockstep
at step 0. Shuffle builds its list before any move (RNG order).

**Stage 5 (integer folds)** touches no state layout. **Stage 6 (footprint)** gains from merge B's smaller card
and power records only through data footprint, which W1 does not feel (28 L1d misses per action).

**RL7's core asks** (`Combat::energy_cost(card)` via the listener cache, the public intent damage fold, intents
with bases on a caller-owned scratch) are layout-independent and should land before stage 4 as queued; after
merge C the scratch refill is ~400 instructions instead of 2,790 + 534. RL7's cache keys hash whole values
(`Card`, `Monster`, every `Power` field plus `power_data`): after merge B the cost key hashes `CardHot` +
`CardCold` and the intent key hashes `Monster` out of `CreatureHot`; same words, same misses. `Envs::reset`
and `snapshot`/`restore` are already `clone_from`; merge C changes their cost, not their code. The observation
(section 1.5) is the one RL-side change; it lands in merge C in `crates/sts2-rl` (the RL lane's crate, so the
orchestrator schedules it with the core merge or as the next RL task; the old loop keeps working on the
accessors meanwhile).

**Order.** Core lane: 0 → 1 → 2 → 5 → **4A → 4B → 4C** → 6, stage 3 gone as a separate stage. The Silent
snapshot item queued after CQ1 (reuse the snapshot by `clone_from`) can land before stage 1 with the design of
section 1.3 (a lazily boxed snapshot, restore by copy, no per-step move), and merge C then only shrinks the
copy; if it has not landed, merge C includes it.

## 4. Risks (plan section 6), and how each is proven

| # | risk | design | proof |
|---|---|---|---|
| 1 | unbounded piles and cards in combat | spill (2.3); caps from G's spill rate; the typed stop at 65,536 | forced-spill lockstep, sweep and regress (2.3 a-c); the equality test (d); the fuzz target (e) runs 10 min per merge; the bench reports `spills` (W1: 0; S: the survey's count) |
| 2 | summons beyond fixed creature slots; dead creatures' ids stay valid | `creatures` is a region, ids are indices, never reused; cap 16, spill | regress holds Waterfall Giant, Two-Tailed Rat, Living Fog; `sort_enemies_by_slot_name`'s ≤16 assertion stays (enemies cap 8) |
| 3 | removed powers still read mid-pass (PX15 Constrict; Poison on a dead Giant) | `removed` region keeps every instance in removal order; `power_instance` searches live then removed | PX15's regress fixtures; lockstep compares `canonical_digest`, which hashes removed powers per creature |
| 4 | listener order | stage 1 owns it; stage 4 only changes storage; `PileView` keeps Draw top-first for the walk | lockstep compares the legal-action list and order every step; the digest hashes piles in game order |
| 5 | mid-pass membership (`contains_listener` re-check) | untouched by stage 4 | stage 1's oracle; lockstep |
| 6 | RNG call order | no stage 4 code path draws: spill, split and view are pure storage; shuffle unchanged | RNG counters per stream are in the digest, compared every step |
| 7 | `Decimal` math | not touched (stage 5) | n/a; `Orb.value` becomes `PackedDecimal` in storage and `Decimal` on read, exact by construction (PF11's `stale_calculated` did the same) |
| 8 | history read back by content | stays game state in `Cold`, same `CombatHistory` type | history counts in the digest; lockstep |
| 9 | events read back | stay in `Combat`, `Snapshot.events_len` unchanged | `check_replay` compares events in debug builds; lockstep compares events in half the fights |
| 10 | replay (model C): a snapshot missing a cold part a step mutated | the snapshot is `CombatState::clone_from`, which copies every field by construction (`impl_clone_by_fields!` lists them; a missed field fails `cargo check`) | `REPLAY_CHECK` re-runs every step and compares `state == direct` in every debug and `regress` build (so `regress.sh`, `regress_runs.sh`, the sweep, the lockstep all run it); add `debug_assert_eq!(self, source)` at the end of `clone_from` |
| 11 | `Monster::rng` copied instead of advanced | `Hot` is `Clone`, not `Copy`; `Rng` stays `Clone`-only; the slab copies through `clone_from_slice` | type-level; a `Copy` on `Hot` cannot compile |
| 12 | card cold fields read in hot paths | the split of 1.2 keeps every `legal_actions`, draw and turn-end read hot (`keywords`, `cost_base`, `pile`, flags) | merge B's callgrind diff: `card_cold` calls per action in W1 must be about what `data`/`affliction`/`enchantment` reads are today (a handful) |
| 13 | potion slots are positional | fixed positional arrays, `None` for empty | `first_empty_potion_slot` unchanged; lockstep compares pending prompts |
| new | `Eq`/`Hash` over slack | implemented over `get(r)` only | unit test: spilled == unspilled; `REPLAY_CHECK` across a spill |
| new | id narrowing silently wraps | `u16` ids fail loud at `u16::MAX` (2.4); `PowerId` stays `u32` | a unit test creates 65,535 cards and asserts `Status::Unsupported` on the next |
| new | the observation leaks hidden state through raw regions | only `cards` (id order), `powers` (application order), `hand` are exported | RL8's leak test (shuffle the draw pile, reseed, bytes unchanged) stays in `sts2-rl` |

## 5. Merge plan

Every merge: `cargo xtask gate`; bench digests under LTO and `--pgo` identical (fights `ad55be60f5bd8a71`, LA
`85229a762f9361e1` both copy modes, runs `1febef7e3b528fe5`, `af6c19933b438172`, `125847415c9a423a`,
`bfb062b545664dda`, W1-all per character, rl `372d441d8132f8de`, corpus `77720b3f966bdeb9`); `tools/regress.sh
5` and `tools/regress_runs.sh 8`; the strong sweep byte-identical and its spill count recorded;
`tools/replay_history.py --db --sample 300 --jobs 4`; `cargo test -p sts2-kernel --release` with
`KERNEL_DIFF_FIGHTS=2000`; `cargo xtask lockstep` against the merge base (then bump both `rev`s in
`crates/sts2-lockstep/Cargo.toml` to the merge commit); the forced-spill runs of 2.3; instruction counts for
W1, W1-all, LA, runs and RL recorded in `docs/perf.md` with the callgrind diff of W1. Rollback is always the
merge commit; nothing downstream depends on a stage 4 merge except the next one (sts2-rl, sts2-kernel's
`project.rs`, `capacity.rs` and `digest.rs` compile against the accessors and are adapted in the same merge).

**Merge A, "ids, slab, piles" (3 days).** `CardId`/`CreatureId`/`RelicId` to `u16`; `combat/slab.rs` with
`Slab`, `Region`, `Caps` (`DEFAULT`, `MIN`, `STS2_HOT_CAPS`), the spill; `Player`'s five piles, `allies`,
`enemies`, `pets`, `escaped`, `removed_from_deck` become regions of `hot.ids` (the `Hot`/`Cold` split starts here
with everything else still where it is); `Pile` becomes `PileView`; the draw region bottom-first, `draw` pops;
batch moves and `add_cards_to_pile` as stage 3 describes. `move_to_pile_bottom`'s 256-card bitset becomes
`cards.len()`-sized. Content: the 24 pile sites that index or slice a pile directly. Extra gate: the
forced-spill set (first time), the equality test (d). **Gain**: W1 −150 to −250 (stage 3's cluster; the FB2
fixed-array result says the storage itself is neutral; watch `remove_from_pile` staying inline, FB2 saw it go
out of line for +92), LA −3 to −4%.

**Merge B, "cards, creatures, powers" (5 days).** `CardHot`/`CardCold`, `s.card(id)` returning a `CardRef`
view (hot fields by value or reference) and `s.card_cold(id)`; `Power` to 20 B with `owner`; `CreatureHot` in
`hot.creatures`, per-creature power regions, the `removed` region; `relics` as `RelicHot` + `RelicDataTable`;
`orbs` slab; `card_hooks`/`hooks` unions into the header (stage 1's fields move). Content: the ~390 cold-field
sites by sed; `creature(id).powers` reads (5 sites) through `s.powers(id) -> &[Power]`. `bytemuck` added to
sts2-core for the `NoUninit` derives (no `unsafe` in our code). Extra gate: the fuzz target (`fuzz/` sibling
workspace, cargo-fuzz, invariants of 2.3 e) 10 minutes; the kernel differential test re-pointed at the
accessors. **Gain**: W1 −100 to −250 (allocation 370 → ~150: `Vec<Power>` and `Vec<Card>` regrowth and the
per-fight clone's 15 blocks → 7), LA −6 to −8% (creatures and cards copy as 3 memcpys), RL observation card
section −900 per step once `sts2-rl` reads the region.

**Merge C, "copies" (2 days).** `Combat::snapshot` retained and refilled (if the queued Silent item has not
landed); `Pending` without state; `RunRngSet` split (streams in `Hot`, seed string in `Cold`); `CostLists`
flattened; `extra_rewards`, `spawned_enemies`, `deck` through `copy_vec_from`; `debug_assert_eq!(self, source)`
in `clone_from`; `intents()`'s scratch and `Envs` reset measured; the observation of 1.5 in `sts2-rl` (or the
RL lane's follow-up). Extra gate: `--mode rl` with `--envs 1/64/256` same digest; RL7's cache oracle over the
full RL bench in `regress` (`cargo test --profile regress -p sts2-rl -- --ignored`). **Gain**: W1 Ironclad 0;
W1 Silent −2,900 (3,320 → ~400); LA to −10-15% cumulative; RL reset 1,300 → ~300; the replay snapshot for
prompting decks −2,900 per step.

## 6. Expected speed

Baselines (`docs/perf.md`, `docs/reviews/spire-mentor.md`): W1 Ironclad 5,061 instructions per action at
`6b3f9e5`; W1 Silent 8,463; LA 31,800; RL step 18,774 (RL7) or 12,915 with the core asks; reset ~1,300;
`clone_from` 1,210 per copy (640 in 21 memcpys of ~30 instructions, 570 of per-field logic), 5.3 copies per
LA action; the replay snapshot 2,790 + 534 drop per Silent action.

| merge | W1 Ironclad | W1 Silent | LA | RL step (64 envs) | reasoning |
|---|---:|---:|---:|---:|---|
| A | −150 to −250 | same | −3 to −4% | −100 | stage 3's pile cluster (PF14 left `draw` 245, batch 207, shuffle ~200; the plan's −200); the five pile `Vec`s copy as one memcpy (−250 per copy) |
| B | −100 to −250 | same | −6 to −8% | −900 (obs cards) | allocation −200 (`finish_grow` 122 and mimalloc 120 per action are the per-fight clone's regrowth of exact-length `Vec<Power>`, `Vec<Card>`); 16 creatures × 2 `Vec`s + cards → 3 memcpys (−400 per copy) |
| C | 0 ± 50 | −2,900 | −2 to −3% | −300 to −500 (reset, prompting decks' snapshot) | a snapshot becomes ~400 instructions (a 1.2 KB header copy, 6 slab memcpys of 9.4 KB, ~10 cold memcpys mostly empty); the remaining copy logic goes |
| **total** | **−250 to −500 (→ ~4,600-4,800)** | **−3,000 (→ ~5,400)** | **−10 to −15%** | **−1,300 to −1,500** | |

Why the LA number is below the plan's −15-20%: a copy after stage 4 is still ~17 memcpy calls (6 slabs plus the
non-empty cold lists) at ~30 instructions each plus the rep-movsb time of ~13 KB, about 500 instructions
against 1,210 today, so −700 per copy × 5.3 = −3,700 of 31,800 (−12%); the plan counted one memcpy. Cycles
should fall more than instructions on LA (331 L1d misses per action today are the 21 small copies walking
scattered heap blocks; the slabs are contiguous). On W1 no cycle gain beyond instructions is claimed: W1 is
front-end bound and the slab code is about as large as the `Vec` code it replaces. On the RL step the big
items remain RL7's core asks (intents 7,764 → ~500, costs 2,400 → ~300), not stage 4.

## 7. What the implementing agent must not do

- No per-access spill branch (`SmallVec`, `enum { Inline, Heap }`, `match` on a storage variant): PF9 measured
  +4-6% W1 for exactly that; the slab's branch is on `push` only.
- No generic `Engine<S>` or `Hot<K: Caps>`: every content impl would be monomorphised twice (the plan's
  rejected alternative).
- No `unsafe`, no byte arena with casts; `bytemuck`'s safe `NoUninit`/`cast_slice` only, and only for the
  observation's raw regions.
- No history entry shrinking to pay for a `Hot` history list (PF11: a smaller `HistoryEntry` lost 5% on PGO to
  a store-forwarding stall); history stays in `Cold`.
- `Hash`/`Eq` must never see slack; `Debug` must print regions, not `data`.
- Capacities are a table in one place (`Caps`), never literals in engine code.

## 8. Disagreements with the approved plan

1. **History is not a fixed turn list in `Hot`.** The plan (2.1, 2.7, risk 8) puts `history.this_turn` in `Hot`
   with the spill rule. `HistoryEntry` is 64 B; at the capacity the spill rule gives (128: G is 2.7% over 32 and
   0.02% over 64) that is 8 KB, nearly doubling `Hot`, copied on every snapshot and lookahead copy, while a
   turn holds 10-30 entries (1-2 KB) in normal play. Nothing reads history by offset, and its push is a `Vec`
   push into retained capacity either way. It stays a `Cold` `Vec` with `copy_vec_from`.
2. **Typed slabs, not one slab of `u16`/`u32` words.** The plan's single word slab with typed regions needs
   reinterpretation of word ranges as `CardHot`, `Power`, `Monster` structs: `unsafe` or `bytemuck::Pod` on
   types that hold enums, `Option`s and `&'static str`. Six typed `Vec`-backed slabs are safe Rust, keep the
   element types, and cost six memcpys per copy instead of one: ~150 instructions, against a copy that is
   ~1,200 today. Said plainly: a full snapshot is about 17 bounded memcpys, not one; the gain is the logic that
   disappears and the allocations that never happen.
3. **Stage 3 is merge A of stage 4, not a stage of its own.** Both rewrite the same ~700 lines (`Pile`,
   `remove_from_pile`, `move_to_pile_bottom`, `draw_top_to_hand`, `insert_into_pile`, the batch moves in
   `commands/piles.rs`); doing it on `Vec` first and again on regions a week later is the same work twice with
   two lockstep rounds. The order becomes 0 → 1 → 2 → 5 → 4A → 4B → 4C → 6.
4. **Capacities by spill rate, not ×2 of the survey maximum.** RL0 already said so; the rerun confirms it:
   the ×2 rule gives 512-card regions and 512-card piles (8 KB + 4 KB) because single Waterfall Giant fights set
   the maxima. `Hot` is ~9.4 KB at the chosen capacities, above the plan's 4-8 KB; two KB of that is
   `Monster` × 16, the lever if it has to shrink (8.1 saves another 8 KB against the plan's layout).
5. **`PowerId` stays `u32`.** The plan narrows every id. Power ids are unique per combat and never indices;
   a 10,000-action episode with six enemies applying a few powers per enemy turn creates tens of thousands of
   them, so `u16` would wrap inside the RL bench's own cap and silently alias two instances in
   `power_instance`. `Power` is 20 B with a `u32` id and 16 B without; 64 entries × 4 B is not worth the risk.
   `CardId`/`CreatureId` at `u16` are different: they are indices with a loud stop at the limit (2.4).
6. **The observation's records.** The plan's 6 B card and 4 B power records cannot be a memcpy of `Hot`; the
   memcpy exists only if the trainer takes the engine's 16 B and 20 B records. Recommended, with the loop form
   as the fallback (~1,300 against ~200 instructions per step; both are correct and leak-free).
7. **Removed powers are one owner-tagged region**, not per-creature inline slots with a cold spill: same
   semantics (removal order per creature is the filtered order), one region instead of 16, and the per-creature
   spill rate (G: 9,390 ppm over 8) would otherwise force 16 or 32 slots per creature.
8. **The Silent snapshot fix is not `Option<Snapshot>` in `Combat` moved per step** (RL0's prototype, +2.3% on
   non-prompting decks) but a lazily allocated `Box<CombatState>` restored by `clone_from`; it can land before
   stage 1 as queued.

## 9. Measurements made for this document (scratch, reverted)

- Type sizes (`size_of`, debug build, `f171aa5`): `CombatState` 1,344 B of headers; `Combat` 1,448; `Player`
  280; `Creature` 256; `Card` **160**; `Power` 24; `Relic` **208** (`RelicData` 200); `Potion` 8; `Orb` 48
  (`Decimal` 32); `Monster` **128** (`MonsterKind` 16, `MoveMachine` 64, `Rng` 40); `RunRngSet` 512;
  `HistoryEntry` **64** (`HistoryKind` 48); `CombatHistory` 224; `CardData` 36; `Keywords` 1; `EnergyCost` 24;
  `StarCost` 24; `Affliction` 8; `Enchantment` 12; `PowerData` 144; `DeckCard` 20; `HookSet` 16 (126 hooks);
  `PowerKinds` 32 (242 kinds); `CardKind` 2 (558 kinds); `Entry` 32; `Listener` 12; `DecisionRequest` 80.
- Capacity survey rerun (section 2.1): `cargo build --profile regress -p sts2-diff`, then `target/regress/
  sts2-diff sweep --runs 200 --policy greedy --capacities --threads 8` (G) and `--runs 1000 --strong
  --capacities --threads 8` (S); outputs were in `target/survey_{G,S}.txt` (deleted with `target/`).
- Community DB (section 2.1): `python3 -I` over `runs` (parsed columns for deck, relics, potions, floors) and
  `raw_json` (`map_point_history[act][point].rooms[].turns_taken` for v0.109-v0.111; `players[0].deck` of the
  largest decks). Read-only (`?mode=ro`).
- Content site counts: `grep -rE '\.(energy_cost|keywords|affliction|enchantment|data|clone_of|star_cost)\b'
  crates/sts2-core/src/content` = 389 lines in 79/51/9/18/0/0/4 files; pile reads in content 89 lines, 65 of
  them `to_vec`/`first`/`last`/`len`/`iter`; `removed_powers` is read in 2 content files and 6 core files.

## 10. After merge A (S4A)

Written against `task/S4A` at `edd32ac` (the Vec slab, `14ce1bc` plus docs) and the new base `92ba688` (main with
the `potion_targets` inline, W1 4,858). Everything below was measured on scratch builds under `target/` of the
S4A worktree (`fast` profile, `perf stat -e instructions:u` over the whole process ÷ actions, W1 = `--mode fights`,
LA = `--mode fights --seeds 1000 --lookahead`, both `--threads 1`; callgrind W1 220 × 10 = 48,018 actions and LA
100 seeds = 109,485 actions, 577,677 copies = 5.28 per LA action). Every scratch build kept both digests
(`ad55be60f5bd8a71`, `85229a762f9361e1`). Nothing was committed; the worktrees were deleted.

### 10.1 What the measurements say

| build | W1 | vs base | LA | vs base |
|---|---:|---:|---:|---:|
| base `92ba688` (= `e86d9d1`) | 4,856 | | 29,297 | |
| `u16` ids only, `19942e0` | 4,898 | +42 | 29,600 | +303 |
| S4A head: Vec slab, `edd32ac` | 4,989 | +133 | 30,332 | +1,035 |
| S4A head + a real `Hot::clone_from` | 4,989 | +133 | 29,739 | +442 |
| inline arrays, cap 64 (10.2), as is | 4,972 | +116 | 30,031 | +734 |
| … + `take_out_of` `#[inline(always)]` | 4,942 | +86 | 29,819 | +522 |
| … cap 32 | 4,940 | +84 | 29,690 | +393 |
| … + `switch_sides`, `refresh_hooks`, `modify_given`, `can_receive_powers` `#[inline(always)]` | 4,921 | +65 | 29,607 | +310 |

1. **Most of S4A's LA loss is a copy bug, not reads.** `Hot` is `#[derive(Clone)]`, and a derived `Clone` does
   not implement `clone_from`: `hot.clone_from(src)` is `*self = src.clone()`, one `mi_malloc_aligned` and one
   `mi_free` per lookahead copy (callgrind: 577,677 of each, +96 instructions per copy, ~+500 per LA action).
   `impl_clone_by_fields!(Hot { ids })` takes LA from 30,332 to 29,739. Section 1.2 promised "`Clone`, one
   memcpy"; the macro, not the derive, is what delivers it, and nothing checked.
2. **The copy was mis-estimated at the base.** Per copy (callgrind, inclusive): **1,392** instructions, not 1,210:
   16.2 `memcpy` calls = 515; `CombatHistory::clone_from` 174; `HookIndex::listening_cards` (`SmallVec<[CardId;
   4]>`, extended element by element) 52; `CostLists::clone_from` 48; and **595 of per-field logic** (the
   `impl_clone_by_fields` bodies 117, default `clone_from`s that are `*self = x.clone()` 119, `Vec` element loops
   86, `RawVec` capacity tests 65, `Option` matching 64, pointer arithmetic ~90, the rest small). The five
   non-empty id lists were about 3.5 of the 16 memcpys plus ~10 `copy_vec_from` bodies: **~150 per copy**, not
   the −250 section 6 credited merge A with. Measured: the slab with a correct `clone_from` saves ~110 per copy
   (12.7 memcpys instead of 16.2; the 420 B block costs more than the five small copies it replaces).
3. **Reads are taxed, and fixed inline arrays recover most but not all of it.** Prototype (10.2): per-region
   `[u16; CAP]` with a `u16` length, where the one test a slice of a fixed array needs anyway, `len <= CAP`, is
   also the spill branch. Callgrind W1 against the Vec slab: `on_play_wrapper` −42, `move_to_pile` −37, `draw`
   −18, `add_cards_to_pile` −10, `fold_damage` −7; but `take_out_of` fell out of line (+103 W1, +722 LA) until
   forced, and the capacity bytes cost `memcpy` instructions on every copy (cap 64 = 1,280 B: +70 per copy; cap
   32: half). Against the plain `Vec`s of the base the inline lists still cost +15 to +18 in each of
   `move_to_pile`, `add_cards_to_pile`, `legal_actions_into` and `on_play_wrapper`: a `Vec` iteration is two
   loads and no compare, a fixed array is one load and one compare, and the draw pile's bottom-first order and
   the `u16` conversions are on top. **The best merge-A variant is W1 +65 (+1.3%) and LA +1.1% against the base**,
   with the id-list copy saving (~150 per copy, −2.7% of LA) already counted. Merge A cannot be made a gain:
   the id lists are read on every action and copied for ~150 instructions.
4. **`u16` ids alone are +42 W1 and +303 LA** (`damage_into` +18 W1 / +105 LA, the rest spread thin: zero
   extensions and `usize::from` at every `creatures[id]`, `Option<CardId>` layouts). They buy nothing until a
   list of them is inline, which (3) drops. Section 0.2 is withdrawn for the id *types*; `CardHot` (if it comes)
   narrows inside the record.
5. **Stage 3's pile cluster does not exist on W1** (S4A's own finding, confirmed): W1 draw piles hold ~5 cards,
   so the bottom-first draw saves what the push's capacity test costs. `draw` is 202 at the base, 204-222 in
   every variant.
6. **The part of merge C that needs no layout is the biggest item in the stage.** The Silent replay snapshot
   is ~3,320 per action today (`clone` + drop); refilled by `clone_from` it is one copy, ~1,400: **−1,900 per
   Silent action (8,095 → ~6,200, −24%)**, the same for every prompting deck in the RL env, and the `Envs` reset.
   Nothing in merge A was needed for it. Also layout-independent: history 174 → ~60 (its lists through
   `copy_vec_from`), `listening_cards` 52 → ~10 (a fixed `[CardId; 4]` + len, or `copy_vec_from` on a `Vec`),
   `CostLists` 48 → ~20 (flattened, section 1.3), `Pending` without state (28 of drop glue per copy).
7. **The `CardHot`/`CardCold` split does not help the copy.** `Cold` is copied too (section 1.3), so splitting
   `Card` moves bytes between two memcpys. It serves the observation's memcpy and locality, and belongs to
   the RL lane's choice (8.6), not to the copy plan.

### 10.2 The storage primitive, if lists go inline

`InlineList<T, const CAP: usize> { len: u16, items: [T; CAP] }` plus an overflow `Option<Box<Vec<T>>>` on the
owning struct (one box for all of a struct's lists). Invariant: `len <= CAP` ⇔ the items are `items[..len]`;
`len > CAP` ⇔ the items are the overflow list (the inline array is stale). Reads: `if len <= CAP {
&items[..len] } else { cold() }`: the compare is the slice's bounds check, LLVM emits no second one. `push`:
`if len < CAP { items[len] = x } else { cold() }`; `pop`/`remove`/`retain` move the list back inline at `len ==
CAP` (cold). `clone_from` is a struct copy plus, only when the source has spilled, a `copy_vec_from` of the
spilled lists. `Eq`/`Hash`/`Debug` over the view, as the slab. Measured on the ten id lists (10.1, rows 5-8):
reads at `Vec` cost where the caller is inlined, copies as part of the owner's memcpy, no allocation.

This relaxes section 7's first must-not. The rule came from PF9's `SmallVec` piles (+4-6% W1): `SmallVec`'s
test is `capacity > inline_capacity` (a load and a compare that the bounds check does not share), its code is
generic and large, and its `clone` copies element by element. The inline list's test *is* the bounds check.
The rule stands for `SmallVec`, `enum { Inline, Heap }` and `match` on a storage variant. Capacities become
`const` generics (a table of `const`s in one place still); the forced-spill runs use a cargo feature
(`tiny-caps`: every capacity 1) on the `regress` build instead of `STS2_HOT_CAPS=min`, since a runtime
capacity would be a load per read. No `unsafe` is needed and none is recommended: the prototype reaches
`Vec`-cost reads in safe code.

### 10.3 Decision: drop merge A; re-scope stage 4 to the copy

**Do not merge `task/S4A`.** Its best form costs W1 and LA; its copy saving is small and is available without
it. Keep the branch for reference (the slab tests, the forced-spill idea, `PileView`); nothing from it is
needed by what follows. The id lists stay `Vec`s; the draw pile stays top-first; ids stay `u32`.

Stage 4 becomes **"the copy"**: three merges, each measured against the per-copy breakdown in 10.1 (2) and
gated by W1 as well as LA.

**Merge A′, "copy hygiene and the snapshot" (2 days; no layout change, no content edit).**
- `Combat::snapshot: Option<Box<CombatState>>`, allocated on first use, refilled by `clone_from`, restored by
  `state.clone_from(&snapshot)` (8.8); `Pending` without state.
- `CombatHistory`'s lists and `listening_cards` through `copy_vec_from` / a fixed array; `CostLists` flattened
  (section 1.3); every `clone_from` on the copy path field-wise (`impl_clone_by_fields!`), none derived.
- **A no-allocation test for `clone_from`**: a `#[cfg(test)]` counting allocator around a lookahead-shaped
  copy loop asserting zero allocations in steady state, and `debug_assert_eq!(self, source)` after
  `clone_from` in `regress` builds. This is the test that would have caught (1).
- Expected: W1 Ironclad 0 (±20); **W1 Silent −1,900**; LA −250 per copy × 5.28 ≈ **−1,300 (−4.5%)**; RL env
  reset and every prompting deck's step −1,900; the run env's `accepts` copies −250 each.

**Merge B′, "heap-free creatures" (4 days).**
- `Creature.powers: InlineList<Power, 8>` and `removed_powers` as one owner-tagged `Vec` on the state (8.7),
  so a `Creature` has no heap of its own; `Vec<Creature>` then copies as one `clone_from_slice` (one memcpy,
  `Rng` stays `Clone`, PF11). `Relic` splits into the walked part (`kind`, `melted`, the hook union: 8 B) and a
  `RelicDataTable` like `PowerDataTable`, so `relics` copies 8 B per relic instead of 208.
- The powers list is read on every hook walk, so this merge carries the read risk merge A had, with the
  primitive that measured best (10.2) and the W1 gate below. `power_instance` (owner's list, then `removed`
  filtered by owner) and the digest's removal order are as section 1.2 describes.
- Expected per copy: the creature loop ~300 → ~60, relics ~40 → ~10, memcpys 16 → ~10: ≈ −350 per copy,
  **LA −1,800 (−6%)**, W1 0 ± 50 (gate: W1 ≤ base + 50 or the merge is reworked; `#[inline(always)]` on the
  small power helpers that fall out of line, as 10.1 (3) needed).

**Merge C′, "the RL readout" (RL lane, 3 days, optional).** `CardHot` as a 16 B `NoUninit` record *beside*
`Card` (filled on `push_card` and on the few fields' writes, or computed into the observation buffer in a
loop: measure both), the observation's cards and powers sections as memcpys (1.5), `Envs` reset on the
retained snapshot. Not a copy item; its gain is the RL step's −900 for the card section, and it needs no
change to content reads. Take it if the RL lane's extraction numbers ask for it.

**Expected totals, replacing section 6:** W1 Ironclad 0 ± 50 (4,858); W1 Silent ~6,200 (−24%); LA 29,300 →
~26,200-26,800 (**−9 to −11%**; the plan's −15-20% needed a copy that is one memcpy, which the per-field
logic and the `Clone`-only `Rng` do not allow without `unsafe`); RL step: the reset and every prompting
deck's step −1,900, plus C′'s −900 per step if taken. Against the other levers: stage 5 (integer folds) is
−150 to −250 W1 for 3 days and should go **before** B′, since B′ is a W1-neutral LA/RL item; A′ goes first
regardless (two days, the largest single gain in the stage, no risk to reads).

### 10.4 For the user to decide

1. Drop merge A (recommended) or land its best variant (+65 W1, +1.1% LA) for structural uniformity.
2. Relax section 7's per-access-branch rule for the shared bounds/spill branch of 10.2 (needed by B′). No
   `unsafe` is asked for.
3. Capacities as compile-time constants with a `tiny-caps` feature for the forced-spill gate, replacing the
   runtime `STS2_HOT_CAPS`.
4. Order: A′ → stage 5 → B′ → C′ (RL lane), or A′ → B′ → stage 5 if LA matters more than W1 this month.

### 10.5 Reproduction

Scratch worktrees under `target/` of the S4A worktree (`git worktree add --detach target/scratch <rev>`),
`cargo build --profile fast -p sts2-bench`; `target/m.sh BIN` (perf stat W1 and LA per action), `target/cg.sh`
and `target/cgla.sh` (callgrind W1 and LA, function tables), `target/cgdiff.py` / `cgdiff2.py` (per-action
diffs). The line-level copy breakdown used `CARGO_PROFILE_FAST_DEBUG=line-tables-only` on the base and
`callgrind_annotate` over `state.rs` and `lib.rs`. The inline-array prototype replaced `combat/slab.rs` behind
the same API (`Slab<T, K, N, CAP>`), with `IdSlab` at `CAP` 64 and 32; its unit test passes at `CAP` 1 and 64.

### 10.5 Decided (user, 2026-10-24)

1. Merge A is dropped; `task/S4A` stays unmerged for reference.
2. Section 7's per-access-branch rule is relaxed only for the inline list's shared bounds/spill branch (B′); no `unsafe`.
3. Capacities are compile-time constants; the forced-spill gate builds with a `tiny-caps` cargo feature (replaces `STS2_HOT_CAPS`).
4. Order: A′ → stage 5 → B′ → C′.
