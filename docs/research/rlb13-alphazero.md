# RLB13: an AlphaZero-style tree, and an A10-trained run policy (paused)

Data and checkpoints: `target/rl-checkpoints/RLB13/` (main checkout). Paused on 2026-10-29 before
dev/test; no gain was accepted, and the default bot is still RLB12's (beam 48, keep 6).

## Built

- **`honest/az.rs`** (`--controller az`): one PUCT tree per decision from the visible position.
  Each simulation samples a blindfolded world. Nodes are keyed by what the player sees
  (`tree::visible_key`, shared with RLB7), so play orders that reach one position share a node,
  and nodes hold no `Combat`. An edge's outcomes (draws, rolls, next hand) widen progressively
  (`k·(n+1)^α`, a wider `end_k` on end-turn). Past the limit, a simulation folds into an existing
  outcome by replaying that outcome's world seed and key path from the root. Leaves use the
  value net: P(win) + 0.3 × HP kept, wins 1 + 0.005 + HP, losses 0, minus 0.005 per potion.
  The horizon is `--turns 4`. Optional: the policy-head prior (`--prior-mix`), a turn-score
  prior (`--turn-prior`), and batched leaves with virtual loss (`--az-batch`, no speedup).
  Defaults: c 0.3, uniform prior, bosses only (`--az-in`), 4,000 boss sims. Other fights use
  the value beam.
- Leak gate: `the_az_search_does_not_read_hidden_state` (with folds/replays) and
  `the_az_bot_does_not_read_hidden_state` pass.
- `fights|az-collect --starts FILE` reads recorded real-bot starts (`RLB12/vc-a/starts.jsonl`);
  `--only act3`.
- `az-collect`: each tree decision's observation, root visits by action index, and outcome.
  `python/rlb13_az_ei.py` fits the policy head to the visits and the value to outcomes
  (`--anchor` holds the value heads at the start net's outputs).

## Results

Fight screens (paired starts; HP is HP left):

| starts | beam | tree | notes |
|---|---|---|---|
| elites, RLB6-driven, 159 | 152, 51.0 HP | policy prior: −2 to −2.7 HP at any c 0.03–1.5; uniform prior S1600/S3200: −0.04/−0.06 HP | S200 −5.3, S800 −2.7, S12800 −1.1 with the policy prior; horizon t1/t2/t4 equal; end-turn leaves −3.9; turn-score prior −0.5 to −1.2 |
| bosses, RLB6-driven, 176 | 136 | B1600 145, **B4000 151** (disc 5/20, +3.1 HP), B8000 152 | knee ≈ 4,000 sims; 21 s vs 10 s per fight |
| act-3 bosses, real bot (vc-a, 1,022,000+), 290 | 272 | B4000 272 (−0.7±1.3 HP); EI prior m0.5 270, m0 273 | tree 31 s/fight vs 24 s (loaded machine) |

The policy head is a bad prior: on tree positions its CE to the visits was 9.5 nats.
The boss gain appears only on the RLB6-driven decks, which are weak (77% won). On the real bot's
starts (94% won), the tree ties the beam.

**Whole runs, paired against RLB12 on train seeds** (base `base.tsv` = RLB12 `train-b48k6` +
`train-base2`: 1,849/2,000 = 92.45%). The tree is `bin/sts2-diff-az2`, bosses at 4,000 sims,
in chunks `train-az2a` (prefix to 1,000,296), `train-az2c` (1,000,297+, stopped) and
`train-az2b` (1,001,000–1,001,999, complete):
**n 1,601: base 1,486, tree 1,478, discordant 78/70 (p 0.57).** Flat; the deaths in both
are act-3 boss coin flips. A B1600 run stopped at 60 pairs (55/55).

**Expert iteration, round 1:** 31.7k tree decisions from 500 runs' boss fights
(`ei1-data`, seeds 1,020,000–1,020,499). Policy CE went 9.48 → 1.52 and top-1 0.35 → 0.41
(held out). The value heads overfit without `--anchor` (0.725 → 0.754); `ei1a/e8.bin` is anchored.
As the tree's prior it gave no gain (table above), so there was no second round.

**A10 run policy** (`ppo-a10`): warm start from builtin, cheap value search (2,2,8 worlds,
beam 8) at A10, lr 5e-5, 1,024 runs per iteration, 6 jobs, about 6k runs/h. It stopped at
iteration 27. A10 dev (300 runs, greedy): 21 at start, 23 at it10, 25 at it20; mean floor
34.3 → 35.9. **it20 at A0, paired** (`train-a10it20`, stopped): n 268, base 246, variant 244,
discordant 13/11.

## Resume

```
D=target/rl-checkpoints/RLB13; V="--value builtin --value-leaves --value-hp 300"
B=target/release/sts2-diff
$B bot --first 1000000 --count 2000 --run-policy $D/a10-it20.bin $V --out DIR      # A10 at A0
STS2_DIFF_BIN=$D/bin/sts2-diff-base .venv/bin/python python/train_run_ppo.py --out $D/ppo-a10 \
    --resume $D/ppo-a10/last.pt --lr 5e-5 --lr-end 1e-5 --iters 80 --eval-every 10 \
    --eval-runs 300 --search 2,2,8,8 --fight-args "--ascension 10 $V" --jobs 6   # A10 training
$B bot --first 1000297 --count 703 --run-policy builtin --controller az $V --out DIR # tree A/B
python3 $D/paired.py $D/base.tsv $D/train-az2a/progress.tsv $D/train-az2b/rows.tsv DIR/progress.tsv
$B fights --starts RLB12/vc-a/starts.jsonl --first 1022000 --count 300 --only act3 \
    --controller az $V --out FILE                                                   # real screen
$B az-collect --starts RLB12/vc-a/starts.jsonl --first 1020000 --count 500 --only boss \
    --controller az $V --out $D/ei1-data
.venv/bin/python python/rlb13_az_ei.py --data $D/ei1-data --init RLB11/run2/last.pt \
    --out $D/ei1a --eval-runs 40 --epochs 8 --lr 1e-4 --anchor
```

## Next

The tree and the beam saturate at the same strength on the real bot's bosses, so the shared
limit is likely the value net's leaf ranking, not search breadth. Next:

1. Finish the A10 policy's A0 A/B (≥1,000 pairs). Train it longer: spire-mentor's was 700
   iterations; ours stopped at 27.
2. Train a value net on the tree's own outcomes, with HP terms, on real-bot boss starts. Then
   screen on `--only act3` real starts, not RLB6-driven ones, which overstate gains.
