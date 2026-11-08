# RLB17: expert iteration for boss fights (negative)

**Result:** negative. Round 1 gained about a point on the hard benchmark (not significant) and
round 2 gained nothing, so expert iteration on the value net does not compound. The default is
unchanged; the code is kept as tools. Data and
nets: `target/rl-checkpoints/RLB17/` (main checkout). Code: `main.rs` (`search-collect`,
`--reseed-from`), `honest/search.rs` (`scored_with`, `Scored`), `honest/rollout.rs` (the v4
playout net: the hand summed into the hidden layer), `python/rlb17_value.py`,
`python/rlb17_policy.py`.

## Data (round 0)

`search-collect` plays recorded starts with the value search and records, at every searched
decision, the value net's observation, the playout policy's view (features, candidates, hand),
each candidate's search score (best total per world, in fight wins), the search's root value `q`
and the fight's outcome. Round 0: the default search on the 1,922 act-3 boss starts of seeds
1,020,000–1,021,999 (`RLB12/vc-a/starts.jsonl`), 3 copies each (k = 0–2): **5,766 fights, 95.4%
won, 213,088 decisions** (`r0data`), 2 hours on 16 cores.

Headroom: at the same positions the search's `q` predicts the fight's outcome far better than
the net's own leaf value s = P(win) + 0.3 × HP kept (stratified by boss and turn, in-sample):
AUC **0.950 vs 0.900** (Test Subject 0.936 vs 0.869, turn 1 0.904 vs 0.829). The search is
pessimistic in every phase (mean drift of `q` between turn starts +0.03; Queen with the amalgam
alive +0.054); no phase is over-optimistic.

## Benchmark

`bench.sh NAME BIN VALUE ROLLOUT hard`: RLB16's benchmark (170 hard Queen and Test Subject starts,
8 reseeds each, 1,360 fights), paired against `RLB16/bench/base.tsv`. This build reproduces those
rows exactly (32 of 32 checked). Held-out set: the other 785 act-3 boss starts of seeds
1,022,000–1,022,999 (`bench/holdout.jsonl`), 2 reseeds; the default wins 1,551 of 1,570
(98.8%), 71.8 HP left.

Round 1 (value nets fine-tuned from `RLB11/run2/last.pt` on `r0data`, all decisions, 3 epochs,
lr 1e-4; the playouts stay v2b):

| net | Queen (840) | Test Subject (520) | all (1,360) | disc. | p | HP left |
|---|---|---|---|---|---|---|
| default | 85.0% | 75.8% | 81.5% | – | – | 34.4 |
| **v1**: outcomes + 10 × (s − q)² | 83.2% (p 0.13) | **81.0%** (39/66, p 0.011) | 82.4% | 90/102 | 0.43 | 35.9 |
| v1q: 0.3 × outcomes + 30 × (s − q)² | 85.0% (36/36) | 78.8% (p 0.12) | 82.6% | 74/90 | 0.24 | 35.5 |

Both gain about a point overall, which is not significant. Test Subject gains in both; v1 pays for
it on Queen and v1q does not. Held-out q error fell 0.0129 → 0.0093 (v1) and 0.0079 (v1q), but
held-out outcome AUC did not move (it rests on about a dozen lost fights). v1z (outcomes only,
VN1's method on this data) was trained but not benchmarked. The held-out set was not run for v1
or v1q.

## Playout distillation (negative)

Cross-entropy to softmax(search score / 0.01) on `r0data` (`rlb17_policy.py`), greedy win rate
from the 955 held-out act-3 starts × 4 worlds (`rollout-eval`):

| net | val regret (fight wins) | greedy win |
|---|---|---|
| v2b (the default playouts, REINFORCE) | ≈0.012 | **74.0%** |
| v4, 64 hidden, hand input, from scratch | 0.0107 | 70.2% |
| v4, 128 hidden, T 0.03 | 0.0114 | 69.7% |

The distilled nets imitate the search better than v2b (lower regret) yet play worse alone.
The search's per-move scores come from planning the whole turn (the cards of the best plan tie,
37% of decisions have a near-tie), which a greedy per-card policy cannot reproduce; the fit stops
at 1.33 nats against a target entropy of 0.97. Not put in the search.

## Round 2

Data from the v1q search: the same starts, copies k = 3–4 (`r1data`, 3,682 of 3,840 fights,
135,122 decisions; the cut last row of `samples.tsv` was trimmed). v2: v1q fine-tuned on
`r0data` + `r1data` (q 30, z 0.3, lr 5e-5, 2 epochs); validation q error 0.0078 (v1q 0.0079).

| net | Queen (840) | Test Subject (520) | all (1,360) | disc. | p | HP left |
|---|---|---|---|---|---|---|
| default | 85.0% | 75.8% | 81.5% | – | – | 34.4 |
| v1q (round 1) | 85.0% | 78.8% | 82.6% | 74/90 | 0.24 | 35.5 |
| v2 (round 2) | 83.5% | 78.7% | 81.6% | 100/102 | 0.94 | 35.0 |

v2 is no better than v1q and level with the default. The search's q targets are already close
to what the net predicts (q error 0.008), so a new round adds little signal; fight outcomes,
the signal that matters, rest on a few hundred lost fights. Not continued.

## Resume

```
cd ~/spirebot/target/rl-checkpoints/RLB17; B=bin/sts2-diff-r1; P=~/spirebot/.venv/bin/python
S=$PWD/../RLB12/vc-a/starts.jsonl; Y=$PWD/python
# round 2 data again in full (or use r1data as it is; --out overwrites, so use a new dir)
systemd-run --user --unit=rlb17-r1data --collect --working-directory=$PWD bash -c \
  "$B search-collect --starts $S --first 1020000 --count 2000 --only act3 --value v1q/e3.bin \
   --value-leaves --value-hp 300 --rollout v2 --value-mix 300 --reseed 5 --reseed-from 3 \
   --jobs 16 --out r1full > r1full.log 2>&1"
# round 2 net (GPU, 2 cores)
systemd-run --user --unit=rlb17-v2 --collect --working-directory=$Y -p CPUQuota=200% bash -c \
  "$P rlb17_value.py --data $PWD/r0data $PWD/r1data --init $PWD/v1q/last.pt --out $PWD/v2 \
   --all --q 30 --z 0.3 --lr 5e-5 --epochs 2 > $PWD/v2.log 2>&1"
# benchmark (hard, then hold), paired
systemd-run --user --unit=rlb17-b-v2 --collect --working-directory=$PWD ./bench.sh v2 $B v2/e2.bin v2 both
python3 bench/cmp.py bench/hard-base-rlb16.tsv bench/hard-v2.tsv
python3 bench/cmp.py bench/hold-base.tsv bench/hold-v2.tsv
# round 0 / round 1 as run
$B search-collect --starts $S --first 1020000 --count 2000 --only act3 --value builtin \
   --value-leaves --value-hp 300 --rollout v2 --value-mix 300 --reseed 3 --jobs 16 --out r0data
$P $Y/rlb17_value.py --data r0data --init ../RLB11/run2/last.pt --out v1 --all --q 10 --lr 1e-4 --epochs 3
$P $Y/rlb17_value.py --data r0data --init ../RLB11/run2/last.pt --out v1q --all --q 30 --z 0.3 --lr 1e-4 --epochs 3
$P $Y/rlb17_policy.py --data r0data --init ../RLB14/v2b.bin --hidden 64 --hard 0 --scratch --hand \
   --epochs 15 --lr 2e-3 --out p1.bin
$B rollout-eval --starts $S --first 1022000 --count 1000 --only act3 --worlds 4 --rollout p1.bin
```

`bench.sh`, `bench/cmp.py` (RLB16's `rlb16_bench_cmp.py`), `bench/holdout.jsonl` and
`bench/hard.jsonl` are in the checkpoint directory. The gate has not been run on this branch.
