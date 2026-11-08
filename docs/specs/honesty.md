# Honesty

The bot may use only what a player sees. A policy or search that reads the draw order, an RNG
stream, an unrolled event outcome or an unseen reward is **clairvoyant** and its win rate is an
upper bound, not a result.

## Visible / hidden

| Visible | Hidden |
|---|---|
| Hand, deck contents, relics, potions, gold, HP | Draw pile order, discard order, exhaust order |
| Enemy HP, block, powers and the next-move intent | The run's and each monster's RNG streams and counters |
| The map, room types and reachable nodes | A `?` room's rolled room before entry |
| Shop stock and prices once the shop is entered | Reward/chest contents before they are offered |
| Card-reward options, event text and options | The outcome an event option rolls (gold, card, damage, fight) |

Card rewards, shop stock, the map and enemy intents are shown by the game, so using them is
honest. "It is in `CombatState`" is not the test; "the player can see it" is.

## Combat

`Combat::blindfold_for_search(seed)` returns a copy with the hidden draw/discard order and the
run and monster RNG streams resampled from `seed` (the piles are put in card order first, so a
world does not depend on the real order); the hand, HP, powers, intents, relics and pile
contents are untouched. A search samples `K` such worlds and aggregates action values. This is
the determinization baseline. The chance-node tree (RLB7, `honest/tree.rs`) samples a world per
simulation and keys its nodes by what the player sees; both start from the visible position,
never from the real hidden state.

The existing `tools` are labelled:

- `sts2-diff reach` and its beam search are **clairvoyant** (it clones the real `Combat` and runs
  the enemy turn). Keep it as a diagnostic upper bound and a source of run-level ideas; never
  report its win rate as the bot's.
- `sts2-diff bot` (RLB6) is the honest bot: its search plays only on blindfolded worlds and its
  run choices read only the screen; `crates/sts2-diff/tests/it/honest.rs` is its action-leak gate,
  with cases for the chance-node tree and the learned playout policy (RLB7).
- `sts2-rl` combat observations already pass the hidden-state leak test
  (`crates/sts2-rl/tests/leak.rs`), so the combat PPO policy is honest by construction.
- `sts2-rl` run observations pass the same test; a run policy trained on them is honest if it
  does not inspect the run state directly. The RLB8 run policy (`honest/policy.rs`) reads only
  the observation bytes and the legal list, and the action-leak gate runs it
  (`the_run_policy_does_not_read_hidden_state`).

## Run decisions

Map, card-reward, shop and rest choices use the visible state. Event and ancient options must use
their text/expected values or a learned policy; cloning the run and `apply`-ing an option to see
what it rolls is cheating. If an outcome is not modelled honestly yet, treat it as unknown and use
a conservative visible heuristic.

## Tests before a result

1. **Observation leak**: mutate hidden state (draw order, RNG streams, event RNG, relic bags) and
   assert the observation bytes are unchanged. `crates/sts2-rl/tests/leak.rs` does this for combat
   and run observations; `blindfolding_hidden_state_does_not_change_the_observation` covers the
   new blindfold.
2. **Action leak**: give the policy two states with the same visible information and different
   hidden state; the chosen action (or its distribution) must be identical. This is the gate a new
   search or policy must pass before its win rate is reported.
3. **Replay**: a recorded run is replayed from its seed and actions on the engine; a win is only a
   win if it replays.

## Reporting

Always report the **honest** win rate (visible-information policy) and, separately, the
clairvoyant upper bound. A champion table never mixes them.
