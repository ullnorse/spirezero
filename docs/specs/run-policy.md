# Run policy (RLB8)

A learned policy for every run decision outside a fight, A0 Ironclad, the fights played by the
honest search (`honest/search.rs`). Code: `crates/sts2-diff/src/honest/policy.rs` (features, the
net's forward pass, the record files), `sts2-diff run-collect` (rollouts),
`python/train_run_ppo.py` (BC, PPO, the export). The committed weights are
`crates/sts2-diff/data/run-policy.bin` (provenance in `run-policy.md` beside it);
`sts2-diff bot --run-policy builtin` plays with them.

## Honesty

The policy reads only the run observation (`sts2_rl::run_obs`, covered by `tests/leak.rs`) and
the legal list. Every feature is computed from those bytes; the net never sees `RunState`.
Events are their visible option text keys (hashed) and the event id, ancients their offered
relic ids; no option is applied to a copy. The action-leak gate
(`crates/sts2-diff/tests/it/honest.rs`, `the_run_policy_does_not_read_hidden_state`) runs the
committed weights and a random net.

## Observation

The run observation already had HP, max HP, gold, act, floors, the act boss, deck (card,
upgrade), relics, belt, the map with legal next points, and each screen's offer with upgrades
and prices. RLB8 adds the **enchantment kind** of each deck card (flags bits 1–7, kind + 1; bit 0
is still "enchanted"). Features (`policy::Features`):

- **Tokens** in one embedding table with fixed blocks: card × upgraded, enchantment, relic,
  potion, encounter (boss), event, ancient, event text key (hashed to 2,048 buckets), room
  type, rest option, reward kind, selection purpose, action kind, phase, act.
- **State**: 40 scaled scalars (HP fraction, HP, max HP, gold, act, floors, deck size, basics,
  upgrades, curses, attacks/skills/powers, enchanted cards, relics, potions, removals, the map's
  forward cone from the current point); the deck, relics and belt as sums of token embeddings;
  the boss, phase, act and the screen's context (event, ancient, selection purpose) as tokens.
- **Candidates**: every legal run-level index with up to four tokens (action kind plus the
  offered card/relic/potion/room/option text) and 24 scalars (price and affordability, copies
  already in the deck, a map node's forward cone: min/max elites, rests, shops, unknowns,
  monsters, treasures on the paths to the boss, and rows left). A deck selection with a card
  left to pick offers neither Cancel nor un-picking (the greedy policy looped on Smith, Cancel).

## Net and heads

State MLP (2 × 256, `ReLU`) → `h`; a candidate's score is `MLP(Wh·h + Wc·[Σ tokens, scalars])`
(256 → 128 → 1); the policy is the softmax over the legal candidates, the value a 128-wide head
on `h`. One shared scorer covers every decision type (map, card reward and skip, boss relic,
deck selection, shop item by item, rest, event, ancient, potion claim or skip, bundles,
proceed); the kind token and the context tell them apart. Single-option decisions are taken
without the policy. Evaluation is greedy; training samples. Rust and torch agree on the
log-probabilities to 1e-5 (checked every iteration).

## Reward

`+1` for a win plus potential shaping `0.5·(F' − F)/49` on the total floor, so a run returns
`win + 0.5·floor/49`; γ = 1, GAE λ = 0.95. No HP term. Results are the unshaped win rate.

## Fights in training

`run-collect` plays whole runs on 16 threads with the search at 4/4/16 worlds and beam 12
(RLB7 evaluates with 16/16/64 and 24) and records each run decision. Throughput: 53k runs/hour
with the hand rules, 60k with the BC policy, 34–45k by the end as runs went deeper (median
floor 48). The cheap search wins 17% of dev runs with the hand rules (the full one 25%).

## Results

| A0 Ironclad, full RLB7 search | dev (1,000) | test (200) |
|---|---|---|
| RLB7 hand rules | 249 | 50 |
| RLB8 run policy | 729 | 133 (66.5%) |
| RLB9: Giant fix + 89k more PPO runs | **813** | **157** (78.5%) |

Every win replayed. Deaths are bosses: dev 98 at floor 17 (Waterfall Giant 85 of all deaths),
27 at 33, 95 at 48 (Test Subject 47, Queen 36); 43 elsewhere. Test: 20, 5 and 28 at the three
bosses, 14 elsewhere. A greedy run took 556 actions on average (the hand rules 436).

Cheap-search dev (8,000,000–8,000,499, greedy) through training: hand rules 86, BC 85, PPO
iteration 14: 128, 34: 176, 66: 255, 116: 302, 146: 323 (chosen), 165: 315 (±10 is noise).

- **What mattered**: PPO itself. BC only reproduced the hand rules; the gain came steadily with
  PPO iterations at a small learning rate (KL ≈ 1e-3 per update) and had not flattened by the
  end. Training with 4× fewer worlds than evaluation transferred (it20: 79/200 with the full
  search against the hand rules' 49).
- **Not tried**: HP shaping, larger nets, curriculum; the brief's list of what did not help the
  other bot (more search, learned leaf values, imitation of humans, cross-fight potion
  planning) was skipped.
- **Rollout hazards met**: a missing `may_prompt` (Calamity), Pillage's endless draw loop with
  Hellraiser (`docs/semantics/cards.md`), and a beam that held every child fight of a depth;
  the trainer kills a collection above 12 GB or 20 minutes and retries with new seeds.

### RLB9

- **The Waterfall Giant was a scoring bug.** Killed with Steam Eruption it returns at
  999,999,999 HP to explode; the search summed that HP and avoided the kill. Counting it 0
  (`search::standing_hp`) took it from 8 to 26 of 26 real boss starts; cheap-search dev 324 →
  351/500 with the same policy.
- **Training is flat.** Warm-started PPO (89k runs): cheap-search dev 351 → 359–372/500 from
  iteration 10 on.
- **Search budget does not pay.** On real boss starts (`fights --start-policy`), 128 boss worlds
  won 460/508 against 64's 468, beam 48 1,052/1,106 against 1,053; the learned playout,
  weight sweeps (enemy HP ×0.6/1.5, playout win ×0.5–4), minion weighting and leader-first
  targeting were flat or worse.
- **Boss starts mislead.** A playout that blocks first won 1,371 of 1,440 boss starts against
  1,358 but 1,599 of 2,000 whole train runs against 1,650; judge a fight change on runs.
- **The planner, not hidden information, limits the fights.** A clairvoyant one-world search
  wins 1,382 of 1,451 boss starts, honest 64 worlds 1,377. A one-turn-plan playout lifts the
  clairvoyant search to 1,412 but not the honest one (1,375 at 32 worlds).
- Losses triaged in `docs/research/rlb9-losses.md`: 183 of 187 lost dev seeds are won by one of
  8 sampled honest re-runs (ceiling estimate ≥ 99.7%).

## Reproduce

```
cargo build --release -p sts2-diff
.venv/bin/python python/train_run_ppo.py --out target/rlb8/ppo      # BC, then PPO
# RLB9: warm start from the RLB8 export (META: any run-collect dir, for meta.json)
.venv/bin/python python/train_run_ppo.py --out OUT --init-bin crates/sts2-diff/data/run-policy.bin \
    --meta-from META --iters 400 --lr 4e-5 --lr-end 1e-5 --jobs 13
target/release/sts2-diff fights --first 1097000 --count 512 --only boss \
    --start-policy builtin --variants "w64;w128;c" --out FILE   # real boss starts
target/release/sts2-diff bot --first 8000000 --count 1000 --run-policy builtin --out DIR
target/release/sts2-diff bot --first 9000000 --count 200 --run-policy builtin --out DIR
```

`--run-policy FILE` takes any export (`target/rlb8/ppo/best.bin`); copy one to
`crates/sts2-diff/data/run-policy.bin` to make it the builtin.
