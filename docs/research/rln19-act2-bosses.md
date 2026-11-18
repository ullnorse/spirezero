# RLN19: why act-2 bosses kill NECRO1

Diagnosis only; nothing in training changed. Tool: `python/rln19_act2.py` (harvest, benchmark,
analysis) and two `sts2-rl` examples (`card_types`, `boss_resolver`).

**Checkpoint.** The 770M weights no longer exist: `NECRO1/state.pt` had been overwritten and
nothing pinned it. Everything here uses the copy taken at **781M** (`state.pt` sha256
`d4dd9dc2…`, `ema0.999`). On DEV-1000 it matches D770: act-2 boss reach 132/117/116 (D770
118/120/114) and boss win rate 12/26/59% (D770 14/29/54%).

The checkpoint copy and the raw results (harvests, sampled logs) are in
`~/spirebot/target/rl-checkpoints/RLN19/`.

**Data.** 1,308 act-2 boss fights: DEV-1000 (365 fights) plus SMOKE 23,000,000–23,002,499 (943),
greedy play. Each fight records its entry state and every decision.

## Verdict

**The losses are mostly decided in the fight, not by the entry state.** Only entry HP
predicts the outcome. Deck makeup does not.

| boss | win (all) | win at entry HP ≥ 95% | loss share explained by entry HP (upper bound) | same fight, any of 16 sampled plays wins |
|---|---:|---:|---:|---:|
| Kaiser Crab | 13% | 25% (158 fights) | ≤ 14% | 21% (greedy 14%) |
| The Insatiable | 30% | 38% (179) | ≤ 11% | 43% (greedy 32%) |
| Knowledge Demon | 58% | 71% (202) | ≤ 30% | 70.5% (greedy 56%) |

- Entering every fight at full HP would cut losses by at most 11–30%. That is an upper bound,
  because high-HP runs are better runs overall. The rest, 70–89% of losses, happens with entry
  HP already full.
- That remainder is in-fight play plus deck strength that card counts don't capture. Splitting
  those two needs a stronger player on the same fights. The engine's beam-search `Greedy`
  resolver wins only 3.7% of them (net: 34%), so it cannot serve as that reference.
- Sampled play from the same entry state wins 7–15 pp more fights than greedy play, so part of
  the in-fight loss can be fixed with the net's own moves.

## 1. Entry state (won vs lost, means)

| boss | outcome | n | HP | max HP | HP % | deck | atk | skill | power | upgraded | curses | relics | potions | gold |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Kaiser Crab | won | 53 | 70.3 | 74.3 | 94.6 | 29.5 | 13.9 | 12.2 | 1.8 | 2.8 | 1.6 | 9.6 | 2.1 | 93 |
| Kaiser Crab | lost | 355 | 55.6 | 72.1 | 76.9 | 29.9 | 14.9 | 11.7 | 1.7 | 2.8 | 1.6 | 9.2 | 1.5 | 120 |
| Insatiable | won | 128 | 65.6 | 73.9 | 89.0 | 29.2 | 14.5 | 11.0 | 2.1 | 2.6 | 1.5 | 9.2 | 2.1 | 112 |
| Insatiable | lost | 305 | 58.0 | 73.4 | 78.8 | 29.9 | 15.0 | 11.5 | 1.7 | 2.7 | 1.7 | 9.3 | 1.4 | 128 |
| Knowledge Demon | won | 273 | 63.7 | 72.9 | 87.2 | 30.0 | 15.0 | 11.7 | 1.6 | 3.0 | 1.6 | 9.5 | 1.7 | 110 |
| Knowledge Demon | lost | 194 | 51.4 | 71.1 | 72.6 | 29.4 | 15.1 | 11.3 | 1.4 | 2.6 | 1.6 | 9.2 | 1.3 | 145 |

Out-of-fold AUC for predicting a win from the entry state (5-fold L2 logistic regression;
"HP % only" is the plain AUC of entry HP %):

| boss | all entry features | HP % only | deck counts only |
|---|---:|---:|---:|
| Kaiser Crab | 0.76 | 0.75 | 0.57 |
| Insatiable | 0.70 | 0.63 | 0.57 |
| Knowledge Demon | 0.71 | 0.68 | 0.53 |

Win rate by entry HP %:

| boss | ≥ 95 | 80–95 | 60–80 | < 60 |
|---|---:|---:|---:|---:|
| Kaiser Crab | 25% of 158 | 11% of 70 | 3% of 86 | 2% of 94 |
| Insatiable | 38% of 179 | 36% of 91 | 20% of 90 | 12% of 73 |
| Knowledge Demon | 71% of 202 | 74% of 72 | 52% of 99 | 28% of 94 |

Run-level signals:
- Losing runs arrive with fewer potions (1.3–1.5 vs 1.7–2.1) and **more unspent gold** (+16 to
  +35).
- Decks that still hold the starting Strikes or Defends win 14 pp less (34% vs 48%).
  `DRAIN_POWER` costs 11 pp. No card helps by more than 7 pp.

## 2. Fight play

**Same-state comparison.** The benchmark's fights were played 16 times each with sampled
moves. In the fights where some plays won and some lost, the entry state is identical, so any
difference comes from play. Cells show the won plays' mean, then won minus lost with a 95% CI.

| boss | mixed fights | turns | damage dealt / turn | HP lost / turn | incoming / turn | block / incoming | same, turns with incoming ≥ 20 | Osty HP |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Kaiser Crab | 31 | 12.0 (+2.9±0.9) | 37.8 (+4.3±2.1) | 5.2 (−2.6±0.5) | 20.7 (−2.1±0.6) | 0.53 (+0.09±0.03) | 0.44 (+0.07±0.03) | 3.8 (+0.9±0.5) |
| Insatiable | 47 | 8.9 (+0.8±0.3) | 39.6 (+4.8±1.4) | 6.1 (−2.3±0.5) | 14.6 (−0.6±0.4) | 0.33 (−0.01±0.02) | 0.26 (+0.02±0.03) | 3.2 (−0.2±0.2) |
| Knowledge Demon | 54 | 11.3 (+0.3±0.4) | 40.5 (+5.6±1.5) | 3.8 (−1.5±0.3) | 14.5 (−1.5±0.3) | 0.47 (+0.04±0.02) | 0.34 (+0.06±0.03) | 5.5 (+0.9±0.5) |

How the 854 lost greedy fights end (DEV + SMOKE):

| boss | lost | died holding potions | boss HP left ≤ 25% | boss HP left > 50% | damage dealt / turn, lost vs won |
|---|---:|---:|---:|---:|---:|
| Kaiser Crab | 355 | 1 | 14 | 288 | 31 vs 41 |
| Insatiable | 305 | 14 | 80 | 72 | 29 vs 44 |
| Knowledge Demon | 194 | 2 | 74 | 26 | 28 vs 42 |

Not a problem:
- **Unused energy:** 0.04–0.17 left at turns the net ends itself. 83–94% of turns end only
  when nothing is left to play.
- **Potions:** the first is used on turn 1.1–1.4 and the belt is empty at death in 95–100% of
  losses.
- **Osty:** never dies, but sits at 2–5 HP at turn start.

Boss-specific (greedy, 200 benchmark fights per boss):
- **Kaiser Crab** (Crusher 219 + Rocket 209). All 400 won sampled plays killed Rocket first.
  1,898 of 2,800 lost plays (68%) killed neither arm. The player is Surrounded: the arm behind
  them deals 1.5× damage. The facing is **not in the observation**; the net can only infer it
  from intent damage. Won plays also take 2.1 less incoming per turn.
- **The Insatiable.** Liquify sets Sandpit to 4; it drops by 1 each enemy turn and kills at 0.
  Each Frantic Escape adds 1, and its own cost rises with each play.
  - **46 of 137 losses (34%) are Sandpit kills with HP to spare.**
  - In 18 of those a Frantic Escape was in hand at the last decision; in 6 it was still
    affordable.
  - Frantic Escapes played per fight: 3.6 in wins, 3.8 in losses. Losses play more of them at
    counter 1 (0.53 vs 0.22) and run out.
- **Knowledge Demon.** Losses are close (74 of 194 end with ≤ 25% boss HP left) and come from
  HP: about 14% less damage per turn and less block on the 3×9 and 18-damage turns.

## 3. The fixed act-2 boss benchmark

- **File:** `python/rln19/act2-boss-bench-v1.jsonl.gz` (sha256 `23e19c90…`).
- **Contents:** 200 fights per boss, the first 200 in seed order from the SMOKE harvest above.
  Each fight is a run seed plus the 781M net's action prefix up to the boss fight's first
  decision, and the sha of that obs row.
- **Run:** `bench` replays each prefix (CPU, no net), checks the sha, then plays only the
  fight. It refuses if the engine no longer reproduces a prefix.
- **Libraries:** it reproduces all 600 fights with both HEAD (`d5f1beb`) and the lane's frozen
  library `c27a75633e81a79a`.
- **Determinism:** greedy play is deterministic. Its results equal the harvest outcomes 600/600.

| 781M `ema0.999` | greedy win | Wilson 95% | sampled (16×) win per play | any of 16 |
|---|---:|---:|---:|---:|
| Kaiser Crab | 14.0% | 9.9–19.5 | 12.5% | 21.0% |
| The Insatiable | 32.0% | 25.9–38.8 | 30.0% | 43.0% |
| Knowledge Demon | 56.0% | 49.1–62.7 | 57.1% | 70.5% |
| all 600 | **34.0%** | 30.3–37.9 | 33.2% | 44.8% |

```
cargo build --profile ffi -p sts2-ffi      # or STS2_FFI_LIB=<frozen lib> STS2_FFI_BUILD_ID=<id>
python python/rln19_act2.py bench CKPT --bench python/rln19/act2-boss-bench-v1.jsonl.gz \
    --weights ema0.999 --gpu-window [--samples 16 --log] [--out F]
```

`--gpu-window` pauses the lane, waits for < 3 GB used, plays, then resumes. One greedy run is
about 10 s of GPU (25 s with `--samples 16`). Other commands:
- `resolver --bench …`: the beam-search baseline.
- `paired --samples F`: the same-state table above.
- `detail CKPT --bench …`: card-level plays on CPU.
- `analyze --harvest F…`: the entry and play tables.

## Top losing behaviours

1. **Too little damage per turn in all three fights.** On the same fight, won plays deal 12–16%
   more per turn (+4.3 / +4.8 / +5.6), and on Kaiser Crab, 68% of lost plays never kill an arm.
2. **Under-blocking the big hits on Kaiser Crab and Knowledge Demon.** On turns with ≥ 20
   incoming, won plays cover 6–7 pp more with block and lose 1.5–2.6 less HP per turn. On Kaiser
   Crab, won plays also face 2.1 less incoming per turn, which is consistent with keeping the
   right arm in front.
3. **Mishandling the Insatiable's Sandpit timer.** A third of Insatiable losses are Sandpit kills
   at positive HP.

## Suggested fixes (not done)

- **Show the Kaiser Crab facing in the observation.** Put `SurroundedPower`'s facing in the
  obs (for example as the power's amount or a v2 field). Today the net cannot tell which arm is
  behind it. This needs an engine-side obs change and a retrain or fine-tune.
- **More practice on the hard bosses.** Weight practice toward Kaiser Crab and the Insatiable
  inside `BOSS_ACT2`, or toward act-2 bosses in general. The death rates have not moved since
  500M although practice has run since 575M. Track this benchmark every 50M (10 s of GPU)
  instead of DEV reach.
- **Entry HP is the one entry lever (≤ 11–30% of losses).** Two ways to reach the boss with more
  HP and potions:
  - Inspect the pre-boss rest choice and act-2 elite pathing.
  - Spend the spare 100+ gold at shops, mostly on removing the starting cards.
- **Test whether the fights are winnable.** A stronger in-fight reference (net plus search, or
  more samples) on this benchmark would show how many losses are deck strength rather than play.
  At least 7–15 pp of fights can already be won by the net's own sampled moves.
