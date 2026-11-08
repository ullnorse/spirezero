# rollout.bin

The learned playout policy's weights (`honest/rollout.rs`, about 108 KB). `RolloutNet::builtin`
loads them: the tree's default playouts and priors, and the gate's playout cases.

Provenance (RLB7, train seeds only):

1. `sts2-diff rollout-train --first 1000000 --count 200 --every 4 --controller search --epochs 30`
   (distilled from RLB6's search moves, 26,644 samples).
2. `--only big --iters 200 --per-start 8` on the same seeds, then 200 steps on seeds
   1,000,000–1,000,599, 150 on 1,000,300–1,000,539 and 120 on 1,000,600–1,000,839 (REINFORCE).

Held-out greedy playout win rate (`rollout-eval --first 1090000 --count 200 --only big`): elite
82.7%, boss 50.4% (the priority list: 71.9%, 33.7%). Regenerate after a vocabulary change; with
another vocabulary `builtin` falls back to zero weights.

# rollout-v2.bin

RLB14's v2 playout net (`RolloutNet::v2`, `--rollout v2`, about 420 KB): the v1 features plus
boss-relevant powers, piles and attackers, and 32 hidden units. Provenance (train seeds only,
recorded starts `RLB12/vc-a/starts.jsonl`, act-3 bosses of seeds 1,020,000–1,021,999):

1. REINFORCE from `rollout.bin`: 300 steps on 481 starts, then 25 on all 1,922 (linear).
2. `--hidden 32`: 800 steps, then 1,500 more (`--batch-starts 240 --per-start 16 --rl-lr 0.003`).

Held-out greedy win rate on act-3 boss starts of seeds 1,022,000–1,022,999 (955 starts × 4
worlds): 74.0% (`rollout.bin` 63%, the value net's policy head 75%); every boss 84.5%.
