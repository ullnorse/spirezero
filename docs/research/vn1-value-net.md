# VN1: refitting the combat value net to the real bot's outcomes

Data and checkpoints: `target/rl-checkpoints/VN1/` (main checkout). Trained offline on the GPU
and screened on 2 cores. The result is a tie: `data/value-vn1.bin` (`--value vn1`) is committed
behind a flag, and the default is still RLB11's net.

## Diagnosis (RLB11 net, `python/vn1_diagnose.py`, vc-a boss rows: 65k positions, 8,791 fights)

- **It underrates the killers.** Mean P(win) vs actual: Test Subject 0.80 vs 0.90, Queen 0.82
  vs 0.92, Aeonglass 0.92 vs 0.97. Rows it puts at 0.5–0.8 win 86.5% of the time, and it
  underrates turn 1 (0.90 vs 0.97). It is calibrated above 0.98, where 70% of rows are. HP kept
  on wins is underrated too: 0.58 vs 0.66.
- **Ranking.** Within a (boss, turn) stratum, its leaf score separates won from lost fights at
  AUC 0.898. The player's HP alone gives 0.724.
- **Lost fights.** At the last turn start of the bot's 264 lost boss fights, the mean P(win) is
  0.26. It is above 0.9 in only 3% of them, so the net is rarely confidently wrong late.
- **Price of HP vs damage.** P(win) gained per 10% of max HP on Queen is 0.021 for player HP and
  0.045 for boss HP, and on Test Subject 0.025 and 0.013. A logistic fit of the outcomes on those
  two features (per turn) says the reverse on Queen: 0.049 and 0.030. Retrained nets keep the
  net's ratio, though (Queen 1.5–1.8), and the fit is observational and confounded, so this is a
  hint, not a proven misranking.

## Candidates (`python/vn1_train.py`; seeds 1,022,000–1,022,449 held out)

All start from `RLB11/run2/last.pt` and train for 8 epochs at lr 5e-5 with boss rows ×6, elites
×3 and a KL to the start policy. Metrics are on held-out boss rows.

| | boss BCE | P(win) / won | strat. AUC | HP kept pred / actual |
|---|---|---|---|---|
| RLB11 net | 0.116 | 0.925 / 0.965 | 0.872 | 0.572 / 0.665 |
| **c2** outcomes | **0.083** | 0.965 | 0.886 | 0.648 |
| c3 + pairwise rank loss | 0.093 | 0.942 | 0.884 | 0.645 |
| c4 + rank, widened to 1,024 | 0.094 | 0.942 | 0.884 | 0.645 |
| c5 + rank + RLB13 tree rows | 0.090 | 0.950 | 0.886 (0.888 at e3) | 0.641 |
| c6 rank ×3 | 0.108 | 0.925 | 0.883 | 0.642 |
| c7 widened, 16 epochs, lr 1e-4 | 0.093 | 0.964 | 0.878 | 0.649 |
| c8 TD(λ 0.8) on c2 | 0.083 | 0.966 | 0.885 | 0.648 |

Calibration is fixed after one epoch, and ranking AUC moves by only 0.014. Once boss
calibration is fixed, nothing (width, epochs, rank loss, tree data, TD targets) helps the
held-out loss.

## Screens (beam 48, keep 6, `--value-hp 300`, paired starts from the real bot)

The RLB11 arm on 1,022,000+ is RLB13's `scr/a3-beam`, which this build reproduces exactly.

| starts | RLB11 | c2 | discordant RLB11/c2 | HP left c2 − RLB11 |
|---|---|---|---|---|
| act-3 bosses, 1,022,000–1,022,299 (290) | 272 | 274 | 10/12 | +0.2 ± 1.3 |
| act-3 bosses, 1,022,300–1,022,449 (145) | 133 | 129 | 4/0 | +0.3 ± 1.6 |
| **act-3 total (435)** | **405** | **403** | 14/12 (p 0.85) | +0.3 ± 1.0 |
| act-1 bosses, 1,022,000–1,022,299 (299) | 298 | 298 | 0/0 | +0.3 ± 0.5 |

c5, the best by offline ranking AUC, lost on act 3: 265 vs 272 (discordant 14/7), −2.3 ± 1.1 HP.
On equal machine load the run times are equal.

## What this shows

- Fitting the leaf to the bot's own outcomes fixes calibration and leaves play unchanged. That
  matches RLB12's ei1 (663 runs: 592 vs 585).
- Offline ranking metrics on visited positions do not predict the screen. c5 led them and lost.
  vc-a holds only the line the bot played, so it cannot say whether a sibling leaf would have
  won. That is the label the leaf ranking needs.
- Next: label siblings. Dump each boss decision's kept leaves and play each one out (search or
  clairvoyant) for a win label, then train on pairs. This needs CPU (on the order of 10⁵
  playouts), so it waits until the A10 job ends.

## Reproduce

```
R=target/rl-checkpoints; V=$R/VN1; VL="--value-leaves --value-hp 300"
.venv/bin/python python/vn1_diagnose.py --data $R/RLB12/vc-a --net $R/RLB11/run2/last.pt
.venv/bin/python python/vn1_train.py --data $R/RLB12/vc-a --init $R/RLB11/run2/last.pt \
    --out $V/c2 --epochs 8 --lr 5e-5                     # + --rank 1 [--widen] [--tree $R/RLB13/ei1-data]
target/release/sts2-diff fights --starts $R/RLB12/vc-a/starts.jsonl --first 1022000 --count 450 \
    --only act3 --value vn1 $VL --jobs 2 --out FILE
python3 python/vn1_paired.py BASE FILE
```
