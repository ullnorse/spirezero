# Exhaustive fight solver (tasks SOLV1, SOLV2)

`sts2-diff solve` computes, for every legal action at a fight's current decision, the exact expected HP
loss to the end of the fight and the death probability, by expectimax over the engine's own transitions.
The player maximises the expected final HP (equivalently minimises the expected loss; a death ends with
0 HP, so it costs all HP held at the root). Ties on the expectation go to the lower death probability.

## 1. How chance is exposed

The engine's randomness lives in seeded streams (`RunRngSet`, each monster's `Rng`) and in the draw pile's
order (shuffled on `Rng.Shuffle`). The solver must enumerate outcomes instead of sampling one, with the
engine's own code making every transition. The core change is a **chance script**:

- `Rng` gets a `scripted` flag, stored in the struct's existing padding (`Rng` stays 40 bytes; `Hash`,
  `PartialEq` and the digest ignore it; clones copy it). It is false in normal play, where every call
  takes the old path after one predictable branch on a field already loaded, so normal play is
  byte-identical (the bench digests are the check).
- On a scripted stream every draw asks the thread-local script in `rng::chance` instead of xoshiro:
  `NextInt`/`NextInt(min,max)`/`NextUnsignedInt` (and through them `NextItem`, `NextBool`, list shuffles
  by Fisher-Yates) are one *uniform* chance event over the range; `RandomBranch` and `WeightedNextItem`
  (monster move rolls, weighted picks) are one *categorical* event with probability `w_i / sum(w)`. The
  script holds the outcome indices to return, in call order. When it runs out, the first unanswered call
  is recorded as the step's *need* (its kind and branch count or weights), returns outcome 0, and every
  later call returns 0 too; the solver throws that run away and re-runs the step once per outcome with
  the script extended. Float draws used directly (`NextFloat`, `NextDouble`, `NextGaussianInt`,
  `NextUnsignedLong`) record an `Unsupported` need: the solver refuses rather than guesses.
- **The draw pile is unordered while the `Shuffle` stream is scripted**: a shuffle does not permute
  (the order it leaves is never read), a random insert into the draw pile goes to its bottom, and each
  draw from the top is a chance event that picks which draw-pile card comes up. The need records the
  canonical content key of every draw-pile card at that moment; the solver branches once per distinct
  key with probability `count / pile size`. This equals the engine's distribution: a uniformly shuffled
  pile drawn from the top, with uniform random inserts, is the same as uniform picks without replacement
  (exchangeability), and reshuffles mid-step (an end turn that empties the pile) need no special case.
  A card put on the *top* of the draw pile at a known place breaks exchangeability; it records an
  `Unsupported` need (Headbutt, Thinking Ahead, ...; out of the first slice).
- **Batched draws (SOLV2).** A five-card draw would branch into every ordered sequence of cards. When
  no listener reacts to a drawn card or a pile change (`reactive == false`: the `after_card_drawn*` and
  `after_card_changed_piles*` hooks are all empty), the draw command asks once for all the draws it
  takes in a row from this pile: `count = min(cards still requested, pile size, hand room)`, the
  draws before a reshuffle. The solver branches once per *multiset* of `count` cards with its
  hypergeometric probability `prod C(c_i, h_i) / C(N, count)` and answers with `count` outcomes, one
  draw-pile index per draw (each class's cards in pile order). The engine reads them without asking
  again (`batch_left` in the script); a batch the command does not finish, or any other chance event
  inside a batch, is an `Unsupported` need, so a wrong `count` fails loudly instead of skewing the
  odds. A batch that takes the whole pile is certain and asks nothing. Drawn order inside a batch is
  the solver's (sorted), which is sound for the same reason as the multiset hand: nothing reacts to
  it. With a listener, or with merging off (`--no-merge`, `chance::set_batch_draws(false)`, the
  brute-force check), every draw is its own event with every order enumerated. This replaced SOLV1's
  sorted draw runs (one re-run of the whole step per drawn card): a turn-start draw is now one need
  run plus one run per multiset, about 6× fewer engine steps on the base case.
- A step re-run by the engine itself (the debug replay check, and a step resumed after a player
  decision from its snapshot) replays the same script entries: `Combat::run` marks the script position at
  the step's start and rewinds to it; a pending decision keeps the outcomes its step consumed and puts
  them back in front of the script when the step resumes.
- Monsters created after scripting starts (summons) get a scripted stream too.
- Probabilities are f64 in the solver. Uniform events are exact up to the 2^-53 bias of the engine's
  `(int)(NextDouble() * max)`; categorical events use `w_i / sum(w)` while the engine compares in f32, so
  they are exact to about 1e-7 relative. That is the stated precision of "exact" here.

Chosen over the alternatives: pre-arranging the draw pile per outcome (no core change) cannot reach a
reshuffle inside a step; forcing xoshiro outputs by inverting the state cannot control a whole shuffle
and needs the branch structure anyway.

## 2. The state hash (transposition table)

`CombatState::canonical_hash() -> u128` (core, because history and the side tables are crate-private):
two independent 64-bit lanes over the game state with

- **excluded:** every RNG stream (scripted: their state decides nothing), the id counters
  (`next_power_id`, ...), derived caches (hook index, prompt counts), the run deck and modifiers (fixed
  for the fight), card ids, power ids, potion ids and orb ids (labels only).
- **cards** by a content key: the card's fields with id, pile, deck slot and `clone_of` target
  normalised, plus its cost-list entries. Hand, draw, discard and exhaust are **multisets** (the sum
  of the finalised card keys, mixed in at the pile's place), so play order and draw order merge. The
  play pile keeps its order.
- **creatures** in id order: side, HP, max HP, Block, pet owner, each power's kind, amounts, applier,
  flag and instance data (without its id), monster model fields and move machine (not its `Rng`).
- **player** scalars, relics with their data, potion kinds by slot, orbs, removed powers.
- **history:** the turn key and all counters as stored; this turn's entries as a multiset with card ids
  replaced by content keys and `CardPlay.id` (a play counter, compared only for identity) left out;
  `last_attacks` with content keys. Readers of the turn's entry *order* (none in the first slice's
  content) would make this merge unsound; the brute-force check below catches that.
- Decision-pending states are not merged (their snapshot and partial step would have to be hashed).

Small fields are packed into `u64` words before mixing; each lane is a multiply-xorshift mix per word.
Since SOLV2, values derived `Hash` writes through the `Hasher` (`u8`..`u32`) are packed too: they
fill the current word until the next value does not fit, then the word is mixed with its byte count
(the words depend only on the values and their widths, as derived `Hash` is prefix-free). Card
content hashes are cached per card slot in a thread-local (`card_hashes`): a slot whose normalised
content equals the last one hashed reuses its hash; a fight with cost-list entries skips the cache.

## 3. Scoring, bounds and budget

- Terminal: won → final HP; lost → 0 HP (a death). The value of a node is E[final HP] and P(death)
  under the optimal policy, independent of the root, so the table is shared across roots and solves.
- Every value is an interval `[lo, hi]` for E[final HP] (and P(death)). A node past the turn cap is
  not expanded: it gets `[0, hp_bound]` with `hp_bound = min(max HP, HP + heal_bound)`. Max nodes take
  the max of each bound, chance nodes the weighted sums, so the root interval always contains the true
  value. The report prints the interval and says `exact` only when `lo == hi`; a cut search is never
  reported as exact.
- **Heal bound.** `--heal-bound N` (SOLV1): the most HP the player gains from any state to the end;
  each explored step is checked against it (HP rising more in one step is an error). The default
  (SOLV2) is automatic and *end-only*: `heal_bound` = the sum of the owned relics' `Heal` vars
  (Burning Blood: 6; 0 for Necrobinder's starter), and every explored transition except the winning
  one must not raise the player's HP at all, the winning one by at most `heal_bound`. Along every
  explored line that proves `final HP <= HP + heal_bound`, so the bound is sound wherever the search
  looked; a relic or card that heals mid-fight fails the solve with "pass --heal-bound N" instead of
  bounding wrongly. Without it (SOLV1's CLI default, unbounded) `hp_bound` is the max HP everywhere and
  nothing prunes: SOLV1's binary cannot solve even the Wurm with Burning Blood in 4 M states.
- **Chance bounds (SOLV2).** A chance node's children are bounded by `min(hp_bound, table hi)`: an
  entry's upper bound from a shallower cap (or a pruned search) is still an upper bound, and much
  tighter than the HP. Children are searched most likely first. Together these halved the base case's
  time and states.
- **Alpha pruning.** Max nodes pass each action a bar `max(alpha, best.lo)` and try End turn last; a
  chance node stops when even the upper bounds of the children left cannot clear the bar. A pruned
  value is an upper bound only; the table stores `{value, pruned, cap, work, best}` and reuses an entry
  when it is exact, when its upper bound is below the asking bar, or when it was solved unpruned at a
  cap at least as deep. `best` (SOLV2) is the index of the best action found; a node searched again
  under a deeper cap tries it first (the usual iterative-deepening move ordering). It only orders
  the search; a stale index (a merged state with another hand order) just orders worse.
- **Iterative deepening** on the turn cap: solve with the cap at the current turn + 1, +2, ... up to
  `--max-turns` (default 40) until every root action is exact. Shallow solves fill the table with
  bounds that cut the deep ones; without it the base case hit the 20 M state budget, with it it takes
  83 s.
- **Evicting table (SOLV2).** `--max-states` (default 20 M, about 1.4 GB) is the table's capacity, no
  longer a cut: when it is full, the cheaper half of the entries (by `work`, the visits their search
  took) is dropped, and a dropped state is searched again when met. Values do not change (the test
  `solve_evicting_table_keeps_values` solves with a 64-entry table); only the work grows. `states`
  counts inserts of a key not in the table, so a state searched again after eviction counts again.

## 4. Actions and the CLI

`legal_actions` at each decision, with identical cards in hand (same content key, same target) and
identical potions tried once. `sts2-diff solve --character C --encounter E [--ascension A] [--seed S]
[--hand K,K,..] [--relics R,..] [--play K,..] [--enemy-hp N] [--hp N] [--max-turns T]
[--max-states N] [--heal-bound N] [--no-starter-relic] [--no-merge] [--mc N]` (no `--heal-bound`:
the automatic end-only bound of section 3) prints, per first action, the expected HP loss, death %, and the run's states, visits,
states/s and visits/s. `--no-merge` turns the table and the batched draws off (the brute-force check); `--mc N` plays
the solver's policy N times in the normal engine and prints the mean loss with its 99% CI.

## 5. Validation

- Brute force: the same small fight solved with and without merging gives the same numbers.
- Monte Carlo: the solver's policy played in the unscripted engine from `determinize`d copies of the
  start (fresh shuffle and monster streams per seed); the mean HP loss must sit within the 99% CI of the
  solver's expectation.
- Qualitative (Nico's Lab): Ironclad starter vs Fuzzy Wurm Crawler, hand Strike×3 Bash Defend, after
  Defend: Bash vs Strike+Strike, plain, with Akabeko, with Razor Tooth.
- SOLV2 tests: `solve_two_enemies_merging_equals_brute_force` (Corpse Slugs, batched draws and
  merging against every order, cap 2), `solve_evicting_table_keeps_values` (a 64-entry table gives
  the roomy table's values and evicts), `solve_end_heal_bound_keeps_values` (Burning Blood: the
  automatic end-only bound gives the unbounded solve's values). By hand: Corpse Slugs (8 HP enemies,
  player 12 HP, cap 3) and Necrobinder vs the Wurm (14 HP, player 9 HP, cap 3) equal `--no-merge`.

## 6. Results (SOLV1, release, one core pinned, quiet machine)

- **Brute force** (`--no-merge`) equals the merged solve: Wurm at 30 HP, hand Strike×5, cap 4: End
  turn 4.5198 both; Wurm at 32 HP, player 8 HP: End turn loss 5.797619, death 26.587% both. The test
  `solve_merging_equals_brute_force` pins a cap-3 case.
- **Monte Carlo**: `solve_monte_carlo_matches_expectation` plays the policy 1,000 times (Wurm 50 HP,
  hand Strike×5); the mean loss sits within the 99.9% interval of the exact expectation 4.5198.
- **Qualitative** (Ironclad A0 without Burning Blood, Wurm 57 HP, hand Strike×3 Bash Defend, after
  Defend; expected loss): plain Strike 4.313492 = Bash 4.313492 (End turn 6.22); Akabeko Bash
  2.448 < Strike 4.313; Razor Tooth Strike 1.464 < Bash 2.040. 1.69 M / 0.81 M / 5.93 M states,
  83 / 30 / 511 s.
- **Speed**: about 20 k states/s and 156 k visits/s single-thread on the base case (4 engine steps
  per visit); across the act-1 sweep 13–60 k states/s, 90–180 k visits/s.
- **Where it blows up** (sweep: 11 act-1 weak encounters, 4 M state budget, 240 s): Ironclad solves
  exactly the single-enemy fights that end fast (Wurm, Seapunk, Nibbits, 20–110 s; Shrinker Beetle
  and Sludge Spinner exact on their first actions before the limit). Multi-enemy fights (Slimes,
  Exoskeletons, Bowlbugs, Scrolls of Biting, Corpse Slugs' End turn) hit the budget on the first
  action: targets multiply the actions and each enemy's roll multiplies the chance nodes. Necrobinder
  is exact only where it takes no damage (Shrinker Beetle); elsewhere the first explored action fills
  the budget: Osty, Souls and the generated cards add actions and state per turn, and the deck's
  draw-pile mixes vary more. Devoted Sculptor (162 HP) solves as a certain death for both
  characters; not yet cross-checked by play.
- **Next**: a state budget that evicts instead of cutting (most entries are visited once), merging
  decision-pending states, and symmetric-enemy canonicalisation (identical enemies in any order) for
  the multi-enemy fights.

## 7. Results (SOLV2, release, one core pinned)

Same workload as section 6: the base case is Ironclad without Burning Blood, Wurm 57 HP, hand
Strike×3 Bash Defend, after Defend, `--heal-bound 0`. Values are unchanged at every step (Strike =
Bash 4.313492, End turn 6.220713).

- **Profile before** (callgrind, SOLV1): engine steps 43% (chance re-runs: 4 steps per visit, 31
  per state), combat clones 21%, `canonical_hash`/`card_hash` 16% plus about 10% `malloc`.
- **Kept, in order** (states; wall time; a loaded machine for the first rows, so read the ratios):

  | step | states | steps | time |
  |---|---|---|---|
  | SOLV1 | 1.69 M | 53.0 M | 85 s (20 k states/s) |
  | batched draws (section 1) | | 8.4 M | 6.0× faster |
  | best action first (section 3) | | | 1.5× |
  | packed `Hasher` writes (section 2) | | | 1.1× |
  | table upper bounds + likeliest first in chance nodes | 0.74 M → 0.34 M | 2.85 M | 2.0× |
  | card hash cache, multiset cache, combat pool, hashed `distinct_actions` | 0.34 M | 2.85 M | 3.9 s |
  | pile sums without re-salting, cached draw keys | 0.34 M | 2.85 M | 3.3 s (102 k states/s, 535 k visits/s) |
  | table keyed by the key's low word (no SipHash) | 0.34 M | 2.85 M | 3.23 s (105 k states/s, 550 k visits/s; median of 5, core 12) |

  Callgrind on the last two rows: 43.7 G → 40.1 G instructions. The evicting table and the
  automatic end-only heal bound change no base-case number; they decide the sweep below.
- **Sweep** (11 weak encounters, `--max-states 4000000` as capacity, 240 s, automatic heal bound;
  SOLV1's binary with the same bound, `--heal-bound 6` / `0`, for the before):

  | encounter | Ironclad before | Ironclad now | Necrobinder before | Necrobinder now |
  |---|---|---|---|---|
  | Fuzzy Wurm Crawler | exact 84 s | exact 4.1 s | time | exact 76 s |
  | Seapunk | exact 19 s | exact 0.7 s | cut | exact 115 s |
  | Nibbits | exact 70 s | exact 0.7 s | time | exact 150 s |
  | Shrinker Beetle | cut | exact 14 s | cut | exact 65 s |
  | Sludge Spinner | time | exact 25 s | time | time |
  | Slimes | time | time | cut | time |
  | Exoskeletons | time | time | time | time |
  | Bowlbugs | cut | exact 47 s | cut | time |
  | Scrolls of Biting | cut | exact 224 s | cut | time |
  | Corpse Slugs | cut | exact 24 s | time | time |
  | Devoted Sculptor | exact 13 s | exact 3.5 s | cut | exact 44 s |
  | **exact** | **4/11** | **9/11** | **0/11** | **5/11** |

  "cut" is a budget-limited interval, never counted; "time" hit 240 s. States/s across the solved
  fights: 60–280 k (Scrolls of Biting evicts 60 M entries on the way). Final sweep on cores 13/14.
- **Devoted Sculptor** (a Glory weak encounter, 162 HP): 100% death is correct. Forbidden Incantation
  gives it 9 Ritual, and Savage hits 12 then 9 more each turn (21, 30, 39, ...); a starter deck cannot
  deal 162 before the hits pass 80 HP. Monte Carlo agrees: 30 unscripted games of the solver's
  policy (Ironclad, 80 HP) all die (mean loss 80 ± 0).
- **Dropped**: no iterative deepening (searching at the final cap at once blows up: the shallow
  passes' bounds and best actions are what prune); merging decision-pending states (starter decks
  never prompt, so nothing to gain here).
- **Next**: symmetric-enemy canonicalisation (identical enemies in any order: Slimes,
  Exoskeletons, Scrolls; creature ids inside powers and monster data must be remapped too, or it
  merges unequal states), then threads over the root actions. Necrobinder's open fights fill the
  table on the first action (Osty and Souls add actions every turn).
