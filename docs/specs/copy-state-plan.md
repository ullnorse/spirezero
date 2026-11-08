# A `Copy`, heap-free combat state for all content: plan (task PF9, step 6)

Status (PF11, `docs/perf.md`): stages 1-3 are done, with two changes from the plan. Stage 1 made `MoveMachine` `Copy`
but not `Monster`, whose `Rng` stays `Clone`-only. Stage 2 shrank `Card` from 240 to 160 bytes by packing its
decimals (`PackedDecimal`) instead of moving fields to side tables: the run layer uses standalone `Card`s for vars.
Stage 3 moved `PowerData` to a side table by power id (`Power` 24 bytes). Stages 4-5 are not started. LA's copy is
now mostly allocation: about 14 heap blocks per copy, `malloc` and `free` about a fifth of LA. PF12 removed the allocation instead: `Combat::clone_from` refills a search's scratch copy
in place (0 blocks per copy, LA −27% instructions; `docs/perf.md`), so stage 4 now only saves the per-list copy calls. The goal is a search copy that costs one `memcpy`,
like the kernel's (`docs/kernel-prototype.md`: a 1,960-byte `Fight`), for all content, without giving up
parity.

## Where a copy stands after PF9
`Combat` is 1,416 bytes inline (`CombatState` 1,312, including `RunRngSet` 512). A W1 Act 1 fight 6 actions
in also has about 17 heap blocks, 5.5 KB in total, so one clone means 17 `malloc`s and 17 `free`s. The
blocks, from largest to smallest:

| Block | Bytes (starter deck) | Why |
|---|---:|---|
| `cards` | 2,400 | 10 × `Card` (240 B: `CardData` 80, `stale_calculated: Option<Decimal>` 32+, `EnergyCost`/`StarCost` 24 each) |
| `creatures` | 1,700-1,900 | `Creature` 256 B, plus one `powers` block per creature with powers (`Power` 176 B, `PowerData` 144) |
| history `this_turn` | 500-1,000 | `HistoryEntry` 64 B |
| `deck` | 208 | the run's deck, kept for in-combat deck edits |
| piles, `allies`/`enemies`, relics, potions | 4-48 each | |

In the lookahead bench (every legal action tried on a copy), the clone, its allocations and its drop take
about a quarter of the time: 50,200 instructions per action against the kernel's 11,400.

PF9 has already made `Card`, `Power` and the monster move log `Copy`, moved the rare per-card cost lists to
a side table (`CostLists`), and put the turn's history in a single list.

## What still blocks `Copy`
1. **Growable lists.** These are `cards`, `creatures`, `allies`, `enemies`, the five piles, `pets`, `orbs`,
   `relics`, `potions`, `unported_potions`, each creature's `powers` and `removed_powers`, `deck`,
   `extra_rewards`, `removed_from_deck`, `escaped`, `spawned_enemies`, `CostLists`, and the history's
   `this_turn` and `hp_loss_hits`.
   - The game itself bounds some of them: the hand holds 10 cards, orb slots, potion slots and creature
     slots are capped too.
   - Others have no bound in the game: generated cards (and so the draw, discard and exhaust piles), powers,
     and history.
   - A fixed capacity therefore needs a spill path. Refusing to continue (the kernel's `Unsupported`) breaks
     the "one generalist engine" rule.
2. **Derives only.** `Monster` and `MoveMachine` are plain data that just lack `Copy`. `Creature` and
   `Player` lack it because of item 1.
3. **The `Combat` wrapper.**
   - `events: Option<Vec<Event>>` is off unless asked for.
   - `pending: Option<Box<Pending>>` holds the prompt and decision log.
   - `ListenerCache` is derived data.
   - None of these belongs in the copied part. The cache can be rebuilt lazily after a copy, since it is
     already invalidated on demand.
4. **Size.** At the kernel's 120-card capacity, today's 240-byte `Card` would make a 29 KB state. Copying
   that is slower than today's clone. Cards and powers therefore have to shrink before anything goes inline.

## Staged order (each stage separately measured; digests and the full parity list unchanged)
1. **Derive `Copy` on `Monster` and `MoveMachine`.** This is free.
2. **Compact cards.**
   - Keep a hot part per card of about 16-24 bytes: kind, upgrade, pile, keywords, base costs and the
     flags `legal_actions` reads.
   - Move the rest to side tables keyed by `CardId`, as `CostLists` does: `CardData`, enchantment and
     affliction amounts, `stale_calculated`, `clone_of` and `deck_version`.
   - This is the largest byte saving: 2.4 KB becomes about 0.25 KB at 10 cards. It should also help W1's
     cache footprint.
3. **Compact powers.** Keep amount, kind and uid (8-12 bytes). Move `PowerData` to a side table by power
   uid; only a few powers carry data.
4. **One inline arena with a heap spill.**
   - Put every variable-length list in one fixed-capacity buffer inside the state, as ranges.
   - Set the capacity from W1 and the strong sweep's high-water marks.
   - Overflow moves the state to today's heap layout (an enum with an inline variant and a heap variant).
     Rare fights get slower, but every fight stays exact.
   - PF9 measured the naive version. Inline piles alone (`SmallVec` per pile, or one five-range slab) cost
     **+4% to +6% W1 instructions** for at most 2% on lookahead. Every pile operation paid the inline/heap
     check, and the slab shifted cards between ranges. So piles go last, in the arena, with operations that
     check capacity once per batch.
5. **Wrapper split.** `Combat` would hold a `Copy` core (the state, its streams and the turn flow) plus
   non-copied extras (events, pending prompt, listener cache). A search copies only the core.

**Expected gain.** Lookahead would drop from about 50k to about 35-38k instructions per action: the clone
share goes and a memcpy of about 3 KB is added. W1 changes only through stages 2-3, by an estimated 0-3%.
The kernel's remaining 4× lead on lookahead is mostly its interpreter, not its copy.
