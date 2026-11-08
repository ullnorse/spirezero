# RLB18: a boss-fight deck probe for run decisions; act 1–2 bosses as pure wins

**Result:** negative; the default is unchanged. The deck probe (200 playouts of the act's boss
by the v2 playout net from the run's deck, relics, belt and HP) predicts the act-3 boss fight
very well: AUC **0.961** against RLB16's pre-decided losses (0.907 against all act-3 boss
losses; HP alone 0.611). Used in close run decisions, it raises the probe at the act-3 boss
(0.761 → 0.783) but not wins: **2,000 paired train runs, 1,871 against a pooled base of
1,872.5** (p 0.88). Scoring HP kept as 0 in act 1–2 boss fights, and potions free in boss
fights, changed almost no play: 1,017 and 1,018 against 1,020 of 1,232 reseeded hard act 1–2
boss fights. No dev/test run.

Data: `target/rl-checkpoints/RLB18/` (main checkout). Code: `honest/probe.rs` (the probe,
`after`, `choose_with`), `honest/search.rs` (`ValueUse::mid_boss_hp`, `boss_potion`),
`main.rs` (`deck-probe`, `boss-starts`, `bot --deck-probe`, `--value-mid-boss-hp`,
`--value-boss-potion`). Scripts: `python/rlb18_auc.py`, `python/rlb18_pooled.py`.

## Evaluation

`boss-starts --dirs WINS,LOSSES` replays a bot run's fixtures and writes every boss start as
`collect-starts` does; `fights --starts … --only act3 --reseed 9` plays each act-3 boss start 9
times (as recorded, then 8 copies with the hidden state drawn again). The smoothed line scores
a run as the share of its 9 copies won (0 if it died before the act-3 boss), paired per seed
(t-test). The plain line is paired against each of the default's draws and against their mean
per seed (`python/rlb18_pooled.py`): `RLB16/base.tsv` (943) and `RLB16/salt1` (928) on seeds
1,000,000+, `RLB16/a1` (937) on 1,001,000+.

## Act 1–2 bosses as pure wins

At A0 every act starts on its Ancient, which heals all missing HP (`tests_ancient.rs`,
`act_2_starts_on_its_ancient`), so HP kept at the act 1–2 bosses is worth nothing.
`--value-mid-boss-hp N` sets the leaf's HP weight in those fights; `--value-boss-potion N` the
potion cost in all boss fights. Benchmark: the 157 act 1–2 boss starts of the default's 2,000
runs (seeds 1,000,000–1,001,999) that it lost or left at ≤30% HP, 9 copies each, the 8 fresh
ones counted (1,232 fights, 11 minutes):

| arm | won of 1,232 | disc. base/arm | p | HP left |
|---|---|---|---|---|
| default (HP weight 300) | 1,020 (82.8%) | – | – | 23.7 |
| HP weight 0 | 1,017 | 18/15 | 0.73 | 23.9 |
| HP weight 0, boss potions free | 1,018 | 18/16 | 0.86 | 24.5 |

Only 33 of 1,232 fights change: with value leaves and playouts the HP term (0.3 × HP kept) is
small next to P(win), and the search already plays these fights for the win. No whole-run A/B.

## The deck probe

`honest::probe::probe`: the act's boss (the map's, `next_boss_encounter`) set up from the run
state as the run sets up a fight (deck, relics, belt, HP, max energy, floor, the boss's map
point), on fresh streams from the fixed seed `PROBE{k}` for world k, played to the end by the v2
net (greedy). Candidates share worlds `0..N` (common random numbers). It reads no hidden state:
`the_deck_probe_does_not_read_hidden_state`. 200 playouts take about 40 ms on one core.

`deck-probe --dirs …` probes every boss start of finished runs (2,000 runs, 5,920 boss starts,
15 s on 16 cores). On the default's runs of seeds 1,000,000–1,001,999 (`RLB14/a0-m300`,
`RLB16/base-lost`, `RLB16/a1`), at the act-3 boss, runs that won it against runs that lost it:

| losses | n | AUC | same boss only |
|---|---|---|---|
| all | 74 | 0.907 | 0.895 |
| class a (search-losable) | 53 | 0.886 | 0.870 |
| **class b (pre-decided)** | 21 | **0.961** | 0.957 |
| HP fraction alone, class b | 21 | 0.611 | – |

| probe P(win) | act-3 boss starts | lost |
|---|---|---|
| < 0.2 | 124 | 26.6% |
| 0.2–0.4 | 154 | 13.0% |
| 0.4–0.6 | 186 | 7.0% |
| 0.6–0.8 | 306 | 2.0% |
| ≥ 0.8 | 1,184 | 0.2% |

Act 1 and 2 bosses: AUC 0.981 (11 losses) and 0.955 (18). Mean probe at the act-3 boss: Queen
0.72 (won) vs 0.22 (lost), Test Subject 0.76 vs 0.36, Aeonglass 0.86 vs 0.21. The probe passes
the brief's bar (0.7) by far, so step 4 was built.

## The probe in run decisions

`bot --deck-probe WORLDS,MARGIN,WEIGHT` (`probe::choose_with`): at a run decision where the run
net's top two logits are within MARGIN, and both candidates' effect is plain from the screen
(`probe::after`: a card picked or skipped, a card upgraded or removed by a selection, a shop
card, relic or potion bought, leaving the shop), each candidate's state is probed at full HP
(the deck is compared, not the HP) and WEIGHT × P(win) is added to its logit. Ancient options,
events, rest-site choices and map moves are left to the run net.

Tuning, 200 runs (seeds 1,002,000–1,002,199), 400 playouts per candidate:

| arm | probed / changed per run | won of 200 | paired vs base |
|---|---|---|---|
| base | – | 189 | – |
| margin 3, weight 20 | 20.5 / 1.8 | 192 | p 0.44 |
| margin 3, weight 50 | 20.3 / 2.8 | 188 | p 0.81 |
| **margin 1, weight 20** | 11.1 / 1.4 | 194 | p 0.09 |

Runs take no longer (65 s vs 64 s on 16 runs). Margin 1, weight 20 went to the gate.

## Gate

`--deck-probe 400,1.0,20`, train seeds, `--run-policy builtin`:

| seeds | base draws | variant | paired | pooled |
|---|---|---|---|---|
| 1,000,000 ×1,000 | 943 (`base.tsv`), 928 (`salt1`) | 943 | 28/28 p 1.00; 36/51 p 0.13 | 935.5 → 943, p 0.30 |
| 1,001,000 ×1,000 | 937 (`a1`) | 928 | 28/19, p 0.24 | p 0.19 |
| both | 1,872.5 | **1,871** | 56/47 vs `base.tsv`+`a1`, p 0.43 | −0.8/1000, p 0.88 |

Smoothed (act-3 boss, 9 copies, seeds 1,000,000+): not run; RLB18 was stopped before it finished.

The probe made 1.4 changes a run and moved what it measures: the act-3 boss probe rose from
0.761 to 0.783 (973 paired runs, seeds 1,000,000+) and 0.766 to 0.784 (966, 1,001,000+). The
real fight did not follow: deaths at the act-3 boss (floor 48) went 36 → 40 and 38 → 44. No dev/test:
the gate did not pass.

## Reproduce

```
D=~/spirebot/target/rl-checkpoints; R=$D/RLB18; B=$R/bin/sts2-diff-p4
A="--value builtin --value-leaves --value-hp 300 --rollout v2 --value-mix 300"
$B deck-probe --dirs $D/RLB14/a0-m300/wins,$D/RLB16/base-lost/losses,$D/RLB16/a1/wins,$D/RLB16/a1/losses --out probe.tsv
python3 python/rlb18_auc.py probe.tsv $D/RLB16/classes.tsv $D/RLB16/base.tsv $D/RLB16/a1/progress.tsv
$B boss-starts --dirs $D/RLB14/a0-m300/wins,$D/RLB16/base-lost/losses --out base-boss.jsonl
$R/bench-mid.sh                                            # act 1-2 boss benchmark (bench-mid.jsonl)
python3 python/rlb16_bench_cmp.py mid-base.tsv mid-hp0.tsv mid-hp0p0.tsv
$R/tune.sh                                                 # 200-run tuning
$B bot --first 1000000 --count 1000 $A --run-policy builtin --deck-probe 400,1.0,20 --out w20m1
python3 python/rlb18_pooled.py w20m1/progress.tsv $D/RLB16/base.tsv $D/RLB16/salt1/progress.tsv \
    --smooth smooth-w20m1.tsv smooth-base.tsv
$B fights --starts base-boss.jsonl --first 1000000 --count 1000 --only act3 $A --reseed 9 --out smooth-base.tsv
```

## Next

- The act-3 boss is where runs end: 36–44 of the ~60 deaths per 1,000 runs are at floor 48. The
  probe says most of those fights are already likely losses when they start (pre-decided), the
  rest are lost in the fight. The next step reads traces of the lost act-3 boss fights the
  reseeded benchmark marks as winnable, and looks for a specific search mistake (RLB16's
  benchmark, 170 Queen and Test Subject starts ×8), before any knob.
- The probe is a good signal of deck strength (AUC 0.96). Small nudges in close decisions do not
  move it enough; a larger use would be a training signal for the run policy (reward at each
  act's end), which needs a PPO run.
