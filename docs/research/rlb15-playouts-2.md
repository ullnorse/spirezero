# RLB15: stronger and more playouts, elites

**Result:** no change beats RLB14's default. Better playout nets (v2c, the fight's identity,
the bot's own positions), more playouts per leaf, deeper mixes and more worlds all tie it on 955
act-3 boss starts; the blend in elite fights keeps 1.2 HP a fight but loses the whole-run A/B,
924 against 943 of 1,000 paired train runs (discordant 55/36, p 0.059). The default stays.
Dev and test were not run.

Data: `target/rl-checkpoints/RLB15/` (main checkout). Code: `honest/rollout.rs` (the v3 net,
`--encounters`), `honest/search.rs` (`ValueUse::mix_samples`), `main.rs` (`rollout-train
--search-positions`).

## Playout nets

Greedy from held-out act-3 boss starts (955 starts × 4 worlds, seeds 1,022,000–1,022,999), and
elite starts (1,496 × 4, 1,022,000–1,022,299):

| net | trained | act-3 boss | elite |
|---|---|---|---|
| v2b (`--rollout v2`, the default) | RLB14 | 74.0% | 95.5% |
| v2c | RLB14, never searched | 76.5% | – |
| v3: v2c + which elite or boss fight it is (24 inputs) | +1,500 steps, act-3 boss starts | 75.7% | – |
| sp: v2c on the bot's own positions | 2,161 turn starts of 275 boss fights by the real bot, +1,000 steps | 75.2% | – |
| e3: v2c + fight identity, elite starts | +1,500 steps | – | 97.9% |

The playout policy is stuck near 76%: the fight's identity and the search's own positions did
not lift it. (sp is scored from fight starts; its training positions are mid-fight, where it
wins 85%.)

## Screens

Paired against the default (v2b, mix 300) on the same act-3 boss starts. Screen A is RLB14's
290 (1,022,000–1,022,299); screen B the other 665 (1,022,300–1,022,999), run to confirm.
`sN` is N playouts per end (the greedy one and N − 1 sampled), `tN` stops after N turns and
scores by the net, `w128` doubles the boss worlds.

| variant | A: won of 290 | disc. | B: won of 665 | disc. | boss time |
|---|---|---|---|---|---|
| default (v2b m300) | 278 | – | 643 | – | 1× |
| v2b m300 w128 | 280 | 3/5 | – | | 2.1× |
| v2b m300 s4 | 276 | 7/5 | – | | 1.3× |
| v2b m300 s8 | 279 | 5/6 | – | | 1.6× |
| v2b m500 s8 | 283 | 1/6 | 643 | 8/8 | 1.6–1.7× |
| v2c m300 | 277 | 8/7 | – | | 1× |
| v2c m500 | 273 | 12/7 | – | | 1× |
| v2c m500 t3 | 282 | 3/7 | 637 | 16/10 | 1× |
| v2c m500 t3 s8 | – | | 634 | 20/11 | 1.9× |
| v2d m300 | 277 | 9/8 | – | | 1× |
| v3 m300 | 278 | 5/5 | – | | 1× |
| v3 m300 s4 | 278 | 9/9 | – | | 1.3× |
| sp m500 t3 | – | | 637 | 17/11 | 1× |

Every gain on the 290 vanished on the 665: a 290-fight screen cannot see a 1–2% change
(discordant pairs run 10–30). The default is as good as any mix, depth, net, sample count or
world count tried.

Elites (`--value-mix-in big`), 511 elite fights of seeds 1,022,000–1,022,099:

| variant | won | HP left |
|---|---|---|
| value leaves (no mix) | 510 | 72.9 |
| v2b m300 | 510 | 74.1 |
| e3 m300 | 510 | 74.2 |
| e3 m500 | 509 | 74.0 |

Elites are won already; the blend keeps 1.2 HP a fight more, at no cost in time.

## Whole runs

Paired against RLB14's runs (`base.tsv`, the default) on train seeds 1,000,000–1,000,999, the
default with the blend also in elite fights (`--value-mix-in big`, `a0-el`):

**n 1,000: base 943, variant 924, discordant 55/36, p 0.059.** Every win replayed.

| deaths | base | elite blend |
|---|---|---|
| act-3 bosses (Queen, Test Subject, Aeonglass) | 36 | 45 |
| other bosses | 12 | 18 |
| elites | 8 | 8 |
| other | 1 | 5 |

Elite deaths did not move; the extra losses are in boss fights, which the two arms play alike,
after the runs diverge. Part of the gap may be the base regressing: 943 was RLB14's winning
draw (RLB12's 923 on the same seeds). Either way the elite blend does not help, and the boss
screens gave nothing else worth an A/B (every boss variant within 1% of the default).

## Reproduce

```
D=target/rl-checkpoints/RLB15; R=$D/../RLB14; S=$D/../RLB12/vc-a/starts.jsonl; B=target/release/sts2-diff
V="--value builtin --value-leaves --value-hp 300"
$B rollout-train --starts $S --first 1020000 --count 2000 --rl-count 2000 --only act3 --init $R/v2c.bin \
    --encounters --iters 1500 --per-start 16 --max-starts 2000 --batch-starts 240 --rl-lr 0.0015 \
    --jobs 1 --out v3.bin                                                                # v3 (e3: --only elite)
$B rollout-train --starts $S --first 1020000 --count 2000 --rl-count 2000 --only act3 --init $R/v2c.bin \
    --search-positions 300 $V --iters 1000 --per-start 16 --max-starts 2400 --batch-starts 240 \
    --rl-lr 0.0015 --jobs 16 --out sp.bin                                                # sp
$B fights --starts $S --first 1022300 --count 700 --only act3 $V --rollout $R/v2b.bin \
    --variants "m300;m500 s8" --out s3-v2b.tsv                                           # screen B
$B fights --starts $S --first 1022000 --count 100 --only elite --value-mix-in big $V \
    --rollout $R/v2b.bin --variants "h300;m300" --out s2-el-v2b.tsv                      # elites
$B bot --first 1000000 --count 1000 $V --rollout v2 --value-mix 300 --value-mix-in big \
    --run-policy builtin --out a0-el                                                     # A/B
python3 $D/paired.py $D/base.tsv a0-el/progress.tsv
```

## Next

1. The playout policy is the ceiling of this method and it is stuck at 76% from act-3 boss
   starts. A bigger change is needed to move it: a deeper net over the hand (per-card features,
   not just card keys), or distilling the search's moves on its own positions with a larger
   sample than RLB14's 5,083 moves.
2. Screens of 290 fights mislead (every gain there vanished on 665 more). Screen on all 955
   held-out starts, or more (record more act-3 starts), before an A/B.
3. Re-run the default on train seeds to measure the noise of `base.tsv` (943 vs RLB12's 923):
   whole-run A/Bs of 1,000 runs can only see a 2% change.
4. The deaths are still Queen and Test Subject (19 each here). Look at the lost fights by hand
   (`fights --trace`) for a shared mistake rather than tuning the leaf further.
