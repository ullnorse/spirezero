# RLN36: why NECRO3 dies to bosses

Diagnosis only; no trainer change. NECRO3 100M pin (`state-100M.pt`, sha256 `e69caa9e…`),
`ema0.999`, greedy, lane lib build `e6dd5bfdabd8089f`. Tools: `python/rln36_boss.py` (harvest,
act-3 practice traces, counterfactuals, run-level overrides) and `python/rln36_report.py`
(tables). The harvest reproduces `necro3-100M-dev1000.jsonl` (998/1000 run hashes; the other
2 are the capped runs).

## Verdict

**NECRO3 at 100M is where NECRO1 was at about 250M, and its biggest wall is the act-1 boss, not
act 2.** Boss fights are lost in play from ordinary states. Potions, search and deckbuilding
overrides each move results by at most a few points.

- **Act-1 bosses cause 47% of deaths**, act-2 bosses 15%, act-2 hallway and elite fights 25%,
  act 3 under 2%.
- **Not potions.** About 1 potion is drunk per boss fight and the belt is empty in 99% of
  deaths. Drinking every potion first changes −1 to +3 fights per table.
- **Search at eval adds little.** An honest lookahead (top-k actions × reseeded greedy
  rollouts) adds +3.3 pp in act 1 and +3.9 to +6.7 pp in act 2 (paired CIs include 0).
- **Mostly beyond the net's own play from that state.** 16 sampled plays find a win in 72% of
  act-1 and 21% of act-2 boss fights (greedy: 57% / 8%). Entry HP explains at most 10–24% of
  losses.
- **Deckbuilding is scripted but not the bottleneck.** The run policy never smiths, never removes
  a card and skips 0.5% of card rewards (run entropy 0.76 → 0.11). Forcing Smith at ≥ 70% HP
  gains +0.31 floors; forcing removal of curses, Strikes and Defends loses 0.59; both together
  −0.05.
- **Training gets past the wall.** NECRO1 at 250M had the same boss rates (act-1 55%, act-2
  10%) and reached 68% / 26% at 500M and 83% / 43% at 1B, with the same deck stats (RLN19).
  NECRO3's training floor at 150M (24.1) equals NECRO1's at 500M.
- **The practice mix points the wrong way.** BOSS_ACT1 got 48k practice starts, BOSS_ACT2 202k,
  BOSS_ACT3 68k (one snapshot), while act-1 bosses cause 3× the deaths of act-2 bosses.

| checkpoint (DEV-1000, greedy) | mean floor | act-1 boss won | act-2 boss won | act-3 boss won |
|---|---:|---:|---:|---:|
| NECRO1 250M (`D250r2`) | 20.9 | 454/833 (55%) | 11/110 (10%) | 0/3 |
| NECRO1 500M (`K2`) | 24.1 | 604/891 (68%) | 61/231 (26%) | 3/27 |
| NECRO1 1B (`K3`, 2,000 runs) | 30.7 | 1611/1951 (83%) | 441/1033 (43%) | 19/273 |
| **NECRO3 100M** | 21.4 | 422/894 (47%) | 17/170 (10%) | 0/5 |

## 1. Where runs die (DEV-1000 eval, 998 deaths)

| where | deaths | share |
|---|---:|---:|
| act-1 boss | 472 | 47% |
| act-1 elite / hallway | 71 / 34 | 7% / 3% |
| act-2 boss | 153 | 15% |
| act-2 elite / hallway | 92 / 160 | 9% / 16% |
| act 3 (boss / elite / hallway) | 5 / 2 / 8 | 1.5% |

Boss fights in the harvest (DEV-2000 + SMOKE 23,000,000–23,003,999, 6,000 greedy runs, 6,454
boss fights; the act-3 rows are the 37 real entries, practice starts are in §4):

| act | boss | fights | win % | outcome | HP % | HP | deck | atk | skill | power | curse | upgr | relics | potions | gold |
|---|---|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | Ceremonial Beast | 890 | 54 | won 483 | 92 | 65 | 20.2 | 10.1 | 8.0 | 0.8 | 1.2 | 1.2 | 4.4 | 1.0 | 83 |
| 1 | Ceremonial Beast | 890 | 54 | lost 407 | 81 | 56 | 19.8 | 9.8 | 8.2 | 0.5 | 1.2 | 0.9 | 4.5 | 0.7 | 99 |
| 1 | Lagavulin Matriarch | 872 | 37 | won 324 | 94 | 66 | 20.5 | 10.1 | 8.2 | 0.9 | 1.3 | 1.1 | 4.6 | 1.1 | 77 |
| 1 | Lagavulin Matriarch | 872 | 37 | lost 548 | 85 | 59 | 20.2 | 10.3 | 8.0 | 0.6 | 1.4 | 1.1 | 4.6 | 0.7 | 100 |
| 1 | Soul Fysh | 955 | 67 | won 639 | 91 | 63 | 20.6 | 10.4 | 8.1 | 0.8 | 1.3 | 1.2 | 4.6 | 1.0 | 82 |
| 1 | Soul Fysh | 955 | 67 | lost 316 | 81 | 55 | 19.7 | 9.9 | 8.0 | 0.5 | 1.3 | 1.1 | 4.4 | 0.7 | 103 |
| 1 | The Kin | 900 | 55 | won 498 | 92 | 65 | 20.0 | 9.9 | 8.1 | 0.7 | 1.2 | 1.1 | 4.5 | 1.1 | 82 |
| 1 | The Kin | 900 | 55 | lost 402 | 79 | 55 | 19.8 | 10.0 | 7.9 | 0.6 | 1.2 | 0.8 | 4.4 | 0.6 | 103 |
| 1 | Vantom | 905 | 32 | won 286 | 94 | 67 | 20.0 | 9.7 | 8.1 | 0.9 | 1.2 | 1.1 | 4.3 | 1.0 | 82 |
| 1 | Vantom | 905 | 32 | lost 619 | 85 | 59 | 19.7 | 9.9 | 8.1 | 0.5 | 1.2 | 1.1 | 4.3 | 0.8 | 89 |
| 1 | Waterfall Giant | 892 | 47 | won 423 | 95 | 67 | 20.3 | 9.7 | 8.4 | 0.8 | 1.3 | 1.1 | 4.5 | 1.0 | 83 |
| 1 | Waterfall Giant | 892 | 47 | lost 469 | 84 | 58 | 20.2 | 10.3 | 8.0 | 0.6 | 1.3 | 1.1 | 4.5 | 0.7 | 96 |
| 2 | Kaiser Crab | 316 | 4 | won 14 | 96 | 76 | 29.6 | 15.2 | 11.6 | 1.5 | 1.4 | 2.9 | 8.5 | 1.1 | 43 |
| 2 | Kaiser Crab | 316 | 4 | lost 302 | 76 | 55 | 29.5 | 15.0 | 11.3 | 1.9 | 1.4 | 2.5 | 8.5 | 0.9 | 89 |
| 2 | Knowledge Demon | 324 | 9 | won 28 | 91 | 70 | 28.6 | 14.2 | 11.0 | 2.0 | 1.4 | 2.8 | 9.3 | 1.1 | 56 |
| 2 | Knowledge Demon | 324 | 9 | lost 296 | 75 | 56 | 29.7 | 15.1 | 11.3 | 1.9 | 1.4 | 2.5 | 8.6 | 0.8 | 86 |
| 2 | The Insatiable | 363 | 17 | won 60 | 88 | 66 | 28.8 | 14.4 | 10.9 | 2.0 | 1.5 | 3.1 | 9.2 | 1.1 | 72 |
| 2 | The Insatiable | 363 | 17 | lost 303 | 75 | 56 | 29.5 | 15.0 | 11.3 | 1.8 | 1.4 | 2.5 | 8.6 | 0.8 | 82 |
| 3 | Aeonglass | 13 | 0 | lost 13 | 76 | 64 | 38.9 | 18.7 | 14.7 | 3.2 | 2.4 | 6.2 | 14.8 | 0.7 | 74 |
| 3 | Queen | 12 | 8 | won 1 | 52 | 38 | 51.0 | 35.0 | 13.0 | 1.0 | 2.0 | 10.0 | 12.0 | 1.0 | 99 |
| 3 | Queen | 12 | 8 | lost 11 | 79 | 79 | 36.4 | 18.6 | 12.8 | 3.0 | 1.9 | 6.0 | 13.0 | 0.6 | 66 |
| 3 | Test Subject | 12 | 0 | lost 12 | 88 | 76 | 36.7 | 18.5 | 14.2 | 2.0 | 2.0 | 7.4 | 13.7 | 1.1 | 115 |

Win rate by entry HP %:

| boss | >= 95 | 80-95 | 60-80 | < 60 |
|---|---:|---:|---:|---:|
| Ceremonial Beast | 64% of 500 | 58% of 152 | 39% of 133 | 21% of 105 |
| Lagavulin Matriarch | 44% of 518 | 39% of 145 | 26% of 132 | 6% of 77 |
| Soul Fysh | 73% of 548 | 74% of 151 | 64% of 151 | 32% of 105 |
| The Kin | 66% of 492 | 61% of 157 | 46% of 134 | 15% of 117 |
| Vantom | 39% of 541 | 33% of 142 | 19% of 133 | 6% of 89 |
| Waterfall Giant | 58% of 549 | 45% of 150 | 31% of 118 | 5% of 75 |
| Kaiser Crab | 9% of 121 | 5% of 40 | 2% of 66 | 0% of 89 |
| Knowledge Demon | 18% of 108 | 8% of 65 | 2% of 53 | 3% of 98 |
| The Insatiable | 27% of 125 | 13% of 61 | 14% of 92 | 6% of 85 |
| Aeonglass | 0% of 6 | 0% of 2 | - | 0% of 5 |
| Queen | 0% of 5 | 0% of 2 | 0% of 1 | 25% of 4 |
| Test Subject | 0% of 6 | 0% of 2 | 0% of 4 | - |

How much the entry state explains (5-fold logistic AUC; deck counts include potions and gold).
The last column is an upper bound on the losses removed if every fight won at the ≥ 95%-HP rate:

| boss | n | win | AUC all entry | AUC HP % | AUC deck counts | losses removed at ≥ 95%-HP win rate |
|---|---:|---:|---:|---:|---:|---:|
| Ceremonial Beast | 890 | 54% | 0.76 | 0.65 | 0.66 | ≤ 22% |
| Lagavulin Matriarch | 872 | 37% | 0.75 | 0.62 | 0.69 | ≤ 11% |
| Soul Fysh | 955 | 67% | 0.79 | 0.61 | 0.71 | ≤ 17% |
| The Kin | 900 | 55% | 0.76 | 0.66 | 0.67 | ≤ 24% |
| Vantom | 905 | 32% | 0.75 | 0.62 | 0.66 | ≤ 10% |
| Waterfall Giant | 892 | 47% | 0.75 | 0.65 | 0.64 | ≤ 19% |
| Kaiser Crab | 316 | 4% | 0.82 | 0.75 | 0.58 | ≤ 5% |
| Knowledge Demon | 324 | 9% | 0.67 | 0.72 | 0.58 | ≤ 10% |
| The Insatiable | 363 | 17% | 0.66 | 0.67 | 0.59 | ≤ 13% |

Deck cards that move the win rate (present vs absent, pp; ≥ 10% and ≤ 90% of decks): Lethality
+12 to +25 pp on every act-1 boss, Sculpting Strike −5 to −10 pp on every act-1 boss. In act 2,
no single card moves the win rate by more than 12 pp.

## 2. How boss fights go (greedy; per turn; turns are the engine's)

| boss | outcome | n | turns | dealt/turn | HP lost/turn | incoming/turn | block/incoming | same, incoming>=20 | voluntary end turn | energy left then | cards/turn | potions used | belt at end | boss HP left |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Ceremonial Beast | won | 483 | 10.2 | 27.3 | 6.8 | 14.5 | 0.18 | 0.19 | 0% | 2.00 | 2.94 | 0.98 | 0.02 | 0% |
| Ceremonial Beast | lost | 407 | 10.1 | 17.6 | 5.1 | 16.5 | 0.18 | 0.18 | 0% | nan | 3.06 | 0.69 | 0.00 | 34% |
| Lagavulin Matriarch | won | 324 | 9.6 | 25.4 | 7.2 | 13.2 | 0.19 | 0.17 | 0% | 0.00 | 3.41 | 1.04 | 0.03 | 0% |
| Lagavulin Matriarch | lost | 548 | 10.0 | 16.5 | 5.3 | 14.3 | 0.19 | 0.17 | 0% | 1.33 | 3.46 | 0.66 | 0.00 | 33% |
| Soul Fysh | won | 639 | 10.9 | 21.8 | 6.2 | 9.3 | 0.25 | 0.13 | 0% | 0.50 | 3.36 | 0.90 | 0.03 | 0% |
| Soul Fysh | lost | 316 | 12.3 | 14.1 | 3.9 | 9.9 | 0.21 | 0.12 | 0% | 2.00 | 3.32 | 0.67 | 0.00 | 26% |
| The Kin | won | 498 | 12.0 | 28.2 | 5.7 | 14.2 | 0.33 | 0.24 | 0% | nan | 3.50 | 1.02 | 0.01 | 0% |
| The Kin | lost | 402 | 10.3 | 20.2 | 5.0 | 16.1 | 0.27 | 0.21 | 0% | nan | 3.47 | 0.59 | 0.00 | 54% |
| Vantom | won | 286 | 11.4 | 17.0 | 6.2 | 14.8 | 0.32 | 0.24 | 0% | nan | 3.37 | 0.98 | 0.03 | 0% |
| Vantom | lost | 619 | 10.8 | 9.4 | 4.6 | 15.6 | 0.29 | 0.20 | 0% | nan | 3.30 | 0.75 | 0.00 | 45% |
| Waterfall Giant | won | 423 | 13.5 | 2580.3 | 5.2 | 14.1 | 0.37 | 0.26 | 0% | 1.50 | 3.74 | 0.91 | 0.04 | 0% |
| Waterfall Giant | lost | 469 | 14.6 | 15.1 | 2.9 | 14.2 | 0.31 | 0.21 | 0% | 1.00 | 3.55 | 0.67 | 0.00 | 87% |
| Kaiser Crab | won | 14 | 9.2 | 48.3 | 8.7 | 29.5 | 0.23 | 0.22 | 0% | nan | 4.37 | 1.00 | 0.14 | 0% |
| Kaiser Crab | lost | 302 | 5.9 | 30.4 | 7.7 | 31.5 | 0.21 | 0.20 | 0% | 2.00 | 4.04 | 0.84 | 0.00 | 59% |
| Knowledge Demon | won | 28 | 9.5 | 45.2 | 7.8 | 15.5 | 0.21 | 0.18 | 0% | nan | 3.12 | 1.11 | 0.00 | 0% |
| Knowledge Demon | lost | 296 | 9.3 | 24.2 | 4.9 | 16.4 | 0.24 | 0.20 | 0% | 2.00 | 3.31 | 0.73 | 0.00 | 48% |
| The Insatiable | won | 60 | 8.2 | 43.5 | 8.9 | 16.9 | 0.22 | 0.19 | 0% | nan | 4.05 | 1.02 | 0.05 | 0% |
| The Insatiable | lost | 303 | 6.9 | 27.1 | 5.1 | 16.6 | 0.25 | 0.22 | 0% | nan | 3.83 | 0.76 | 0.02 | 46% |
| Aeonglass | lost | 13 | 6.8 | 32.5 | 8.9 | 22.1 | 0.28 | 0.28 | 6% | 4.38 | 4.16 | 0.77 | 0.00 | 59% |
| Queen | won | 1 | 8.0 | 52.6 | 3.4 | 29.2 | 0.15 | 0.15 | 0% | nan | 3.62 | 0.00 | 0.00 | 100% |
| Queen | lost | 11 | 5.9 | 36.6 | 10.8 | 28.7 | 0.21 | 0.22 | 0% | nan | 3.75 | 0.64 | 0.00 | 80% |
| Test Subject | lost | 12 | 6.2 | 29.9 | 9.3 | 34.0 | 0.25 | 0.23 | 0% | nan | 4.45 | 1.08 | 0.00 | 56% |

Losses: how they end

| boss | lost | died holding potions | boss HP left <= 25% | > 50% | turn of death (median) | died on a turn with incoming >= HP+block |
|---|---:|---:|---:|---:|---:|---:|
| Ceremonial Beast | 407 | 0 | 159 | 75 | 10 | 405 |
| Lagavulin Matriarch | 548 | 0 | 193 | 91 | 10 | 548 |
| Soul Fysh | 316 | 0 | 169 | 35 | 12 | 299 |
| The Kin | 402 | 0 | 72 | 220 | 11 | 395 |
| Vantom | 619 | 0 | 147 | 229 | 11 | 611 |
| Waterfall Giant | 469 | 0 | 39 | 392 | 14 | 466 |
| Kaiser Crab | 302 | 0 | 13 | 212 | 6 | 298 |
| Knowledge Demon | 296 | 1 | 26 | 145 | 10 | 287 |
| The Insatiable | 303 | 6 | 38 | 123 | 7 | 209 |
| Aeonglass | 13 | 0 | 0 | 9 | 7 | 12 |
| Queen | 11 | 0 | 0 | 11 | 6 | 11 |
| Test Subject | 12 | 0 | 1 | 7 | 6 | 11 |

- **The net never ends a turn voluntarily** (0–2% of turns). Turns end when nothing is
  playable, so energy is not wasted on purpose.
- **Potions are not the problem.** About 1 potion is used per fight, and the belt is empty at
  death in 99% of losses. Entry belts are thin: 0.7–0.9 potions in losses against 1.0–1.1 in
  wins, though the bot buys about 1.3 shop potions per act.
- **Deaths come from one big hit at low HP.** In 96% of deaths (Insatiable 69%) the visible
  incoming damage on the last turn is at least the HP at that turn's start. Median HP at the
  last decision is 5–17.
- **The Insatiable**: about 94 of 303 losses (31%) end on a turn whose visible attacks don't
  kill, which fits Sandpit deaths (RLN19: 34%). Losses play Frantic Escape 2.8 times per fight,
  wins 4.2.
- **Kaiser Crab**: 31.5 incoming per turn against 30 dealt; the arms have 428 HP, so the fight
  needs about 14 turns and the bot is dead at turn 6.
- **Act 3**: about 33 dealt and 30 incoming per turn; death at turn 5–7. Test Subject comes back
  with more HP each phase (one trace: 110 → 13 → 209 HP) while its incoming damage climbs from 22
  to 66 by turn 6.
- (The "boss HP left" and "dealt/turn" figures for Waterfall Giant and for won fights are
  unreliable: enemy HP jumps when phases or summons change.)

## 3. Counterfactuals: same state, other play

`rln36_boss.py cf` replays each harvested prefix (sha-checked; greedy reproduces the harvest in
90/90 fights) and plays the fight as:
- `greedy`;
- `potion`: greedy, but every legal potion is drunk first;
- 16 sampled plays at temperature 1 on the real streams (what the net's own moves can reach);
- `look`: the net's top-k actions × n reseeded copies (`Sts2RunEnvs.copy(reseed=…)`, RLN28),
  each played out greedily; play the action with the most wins, then the most HP left.

**Act-1 bosses, 30 fights each (16 samples; look k=3, n=4)**

| boss | fights | greedy | potion | any sample | mean sample | look |
|---|---:|---:|---:|---:|---:|---:|
| Ceremonial Beast | 30 | 21 (70%, 52-83) | 21 (70%, 52-83) | 25 (83%, 66-93) | 61.9% | 23 (77%, 59-88) |
| Lagavulin Matriarch | 30 | 14 (47%, 30-64) | 15 (50%, 33-67) | 20 (67%, 49-81) | 32.3% | 18 (60%, 42-75) |
| Soul Fysh | 30 | 20 (67%, 49-81) | 19 (63%, 46-78) | 25 (83%, 66-93) | 53.1% | 21 (70%, 52-83) |
| The Kin | 30 | 17 (57%, 39-73) | 17 (57%, 39-73) | 21 (70%, 52-83) | 44.4% | 18 (60%, 42-75) |
| Vantom | 30 | 12 (40%, 25-58) | 12 (40%, 25-58) | 16 (53%, 36-70) | 24.2% | 11 (37%, 22-54) |
| Waterfall Giant | 30 | 19 (63%, 46-78) | 19 (63%, 46-78) | 22 (73%, 56-86) | 38.1% | 18 (60%, 42-75) |
| All | 180 | 103 (57%, 50-64) | 103 (57%, 50-64) | 129 (72%, 65-78) | 42.3% | 109 (61%, 53-67) |
lookahead vs greedy, paired: both 93, greedy-only 10, look-only 16, neither 61; diff +3.3 pp (-2.3 to +8.9)

**Act-2 bosses, 30 fights each (16 samples; look k=3, n=4)**

| boss | fights | greedy | potion | any sample | mean sample | look |
|---|---:|---:|---:|---:|---:|---:|
| Kaiser Crab | 30 | 0 (0%, 0-11) | 0 (0%, 0-11) | 1 (3%, 1-17) | 0.4% | 1 (3%, 1-17) |
| Knowledge Demon | 30 | 2 (7%, 2-21) | 2 (7%, 2-21) | 8 (27%, 14-44) | 7.9% | 6 (20%, 10-37) |
| The Insatiable | 30 | 5 (17%, 7-34) | 4 (13%, 5-30) | 10 (33%, 19-51) | 11.7% | 6 (20%, 10-37) |
| All | 90 | 7 (8%, 4-15) | 6 (7%, 3-14) | 19 (21%, 14-31) | 6.7% | 13 (14%, 9-23) |
lookahead vs greedy, paired: both 5, greedy-only 2, look-only 8, neither 75; diff +6.7 pp (-0.6 to +14.6)

**Act-2 bosses, 60 fights each (look k=4, n=8; no samples, so ignore the sample columns)**

| boss | fights | greedy | potion | any sample | mean sample | look |
|---|---:|---:|---:|---:|---:|---:|
| Kaiser Crab | 60 | 0 (0%, 0-6) | 0 (0%, 0-6) | 0 (0%, 0-6) | - | 2 (3%, 1-11) |
| Knowledge Demon | 60 | 9 (15%, 8-26) | 8 (13%, 7-24) | 0 (0%, 0-6) | - | 13 (22%, 13-34) |
| The Insatiable | 60 | 9 (15%, 8-26) | 8 (13%, 7-24) | 0 (0%, 0-6) | - | 10 (17%, 9-28) |
| All | 180 | 18 (10%, 6-15) | 16 (9%, 6-14) | 0 (0%, 0-2) | - | 25 (14%, 10-20) |
lookahead vs greedy, paired: both 11, greedy-only 7, look-only 14, neither 148; diff +3.9 pp (-1.3 to +9.3)

Reading:
- **Greedy-rollout lookahead helps a little and never significantly**: +3.3 pp in act 1, +6.7 and
  +3.9 pp in act 2. That is play the net could learn, but it is small. RLN28's temperature-1
  rollouts lost to greedy; greedy rollouts at least do not.
- **Sampling ceiling**: 16 samples win at least once in 72% of act-1 and 21% of act-2 fights.
  The rest (28% / 79%) the net cannot win from that state by any of its own lines. Kaiser Crab:
  1 of 30 fights; Test Subject: 0 of 36 (act 3, 8 samples).
- **Potions**: −1 to +3 fights out of 76–180 per table. Except in act 3 (+3 of 76), the
  potion-first rule only changes when potions are drunk, not whether.

Not run: NECRO3 on RLN19's fixed act-2 bench (NECRO1's entries). The bench prefixes no longer
replay under the current engine (an illegal action mid-prefix), so a "same deck, other player"
cross test against NECRO1 needs a rebuilt bench.

## 4. Act 3 (practice starts)

`rln36_boss.py act3`: 1,024 envs, 128 BOSS_ACT3 practice slots (max 4 uses per snapshot), the
rest normal runs that capture snapshots. In 50 minutes the normal runs reached the act-3 boss
only **19 times**, so the 80 traced fights come from 19 distinct states. The lane's own pool
holds **1** BOSS_ACT3 snapshot (`stats.jsonl`: `pool.BOSS_ACT3 = 1`, 30,719 starts), so the
lane's act-3 practice replays one deck again and again.

**Act-3 first boss, 76 practice starts (8 samples; no lookahead)**

| boss | fights | greedy | potion | any sample | mean sample | look |
|---|---:|---:|---:|---:|---:|---:|
| Aeonglass | 16 | 4 (25%, 10-49) | 4 (25%, 10-49) | 4 (25%, 10-49) | 13.3% | - |
| Queen | 24 | 0 (0%, 0-14) | 3 (12%, 4-31) | 2 (8%, 2-26) | 1.0% | - |
| Test Subject | 36 | 0 (0%, 0-10) | 0 (0%, 0-10) | 0 (0%, 0-10) | 0.0% | - |
| All | 76 | 4 (5%, 2-13) | 7 (9%, 5-18) | 6 (8%, 4-16) | 3.1% | - |

| boss | outcome | n | turns | dealt/turn | HP lost/turn | incoming/turn | block/incoming | same, incoming>=20 | voluntary end turn | energy left then | cards/turn | potions used | belt at end | boss HP left |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Aeonglass | won | 4 | 5.2 | 81.3 | 9.7 | 19.5 | 0.37 | 0.32 | 0% | nan | 5.02 | 2.00 | 0.00 | 100% |
| Aeonglass | lost | 12 | 7.5 | 33.6 | 6.5 | 21.3 | 0.30 | 0.30 | 2% | 3.00 | 4.40 | 1.33 | 0.00 | 53% |
| Queen | lost | 24 | 6.9 | 35.7 | 9.2 | 28.6 | 0.21 | 0.21 | 1% | 1.00 | 3.58 | 1.00 | 0.00 | 71% |
| Test Subject | lost | 40 | 5.4 | 32.4 | 8.1 | 31.1 | 0.23 | 0.21 | 0% | 9.00 | 4.26 | 0.80 | 0.03 | 55% |

## 5. Run-level decisions (the deck)

Decoded from the action prefixes of the 6,000 harvested runs (per run, up to the boss):

| at | outcome | runs | card rewards taken | skipped | shop cards | shop potions | removals | rest: Heal | rest: Smith |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| act-1 boss | won | 2653 | 6.24 | 0.01 | 1.51 | 1.35 | 0 | 3.29 | 0 |
| act-1 boss | lost | 2761 | 6.25 | 0.01 | 1.26 | 1.34 | 0 | 3.32 | 0 |
| act-2 boss | won | 102 | 12.31 | 0.06 | 3.30 | 2.73 | 0 | 6.52 | 0 |
| act-2 boss | lost | 901 | 13.18 | 0.08 | 2.91 | 2.85 | 0 | 6.37 | 0 |

(Rest decisions count each rest-site choice; won and lost runs build the same way.)

Checked by replaying 80 prefixes up to act 2 against the masks: Smith was legal at every one
of 256 rest decisions (Heal, Smith, …; option order from `Run::rest_options`) and never taken.
At 21% of those rest decisions HP was already ≥ 90%. Card removal was legal in 159 of 214 shop
visits (mean gold at entry 144) and never taken. Skip was legal at all 1,034 card-reward
decisions and taken in about 0.5% of them (0.06–0.08 skips per run).

Overrides (`harvest --smith 0.7 --remove`; combat and every other decision stay the net's,
paired on DEV-2000 seeds):

| variant | runs | overrides | mean floor | act-1 boss reached | act-1 boss won | act-2 boss reached | act-2 boss won | act-3 boss reached | floor vs base, paired (95%) |
|---|---:|---|---:|---:|---:|---:|---:|---:|---|
| o-base | 2000 | {} | 21.52 | 1793 | 861 (48%) | 338 | 37 (11%) | 15 |  |
| o-smith | 2000 | {'smith': 3358} | 21.83 | 1779 | 882 (50%) | 390 | 41 (11%) | 9 | +0.31 (+0.05 to +0.57) |
| o-remove | 2000 | {'remove': 1896} | 19.01 | 1663 | 543 (33%) | 180 | 11 (6%) | 7 | -2.50 (-2.81 to -2.19) |
| o-both | 2000 | {'remove': 1925, 'smith': 2893} | 19.35 | 1664 | 579 (35%) | 215 | 21 (10%) | 14 | -2.17 (-2.51 to -1.83) |
| o-rtarget | 2000 | {'remove': 2021, 'target': 2020} | 20.93 | 1742 | 787 (45%) | 305 | 39 (13%) | 17 | -0.59 (-0.90 to -0.28) |
| o-smith-rtarget | 2000 | {'remove': 2048, 'smith': 3276, 'target': 2047} | 21.46 | 1732 | 828 (48%) | 382 | 51 (13%) | 20 | -0.05 (-0.40 to +0.29) |

`remove`: the net picks the card to remove. It removes its own picks (starters at the act-1
boss 7.93 vs 8.04, deck −1.7) and loses 2.5 floors. `rtarget`: removes a curse, else a Strike,
else a Defend (starters 8.04 → 7.15 by the act-1 boss), and still loses 0.59 floors: removal
costs gold that otherwise buys cards and potions, and the net's combat play was learned on the
scripted decks. Smith alone is the only override that helps, +0.31 floors (about 1.5% of the
gap to NECRO1 at 1B).

[exited with code 0]

## 6. Causes, ranked by share of deaths

| # | cause | share of deaths | evidence | cheapest fix class |
|---|---|---:|---|---|
| 1 | **Act-1 boss fights lost in play from an ordinary state**: the net's damage per turn (about 18 in losses, 27 in wins) is too low for the boss's pace; death by one big hit at turn 10–14 | 47% (472/998) | Entry HP explains ≤ 10–24% of losses. 16 samples win 72% of fights (greedy 57%), so about a third of losses are winnable by the net's own moves; lookahead +3 pp. Practice gives this cell the fewest boss starts: 48k vs 202k act 2, 68k act 3 | **practice mix** (move boss practice to BOSS_ACT1), then more training |
| 2 | **Act-2 bosses out of reach for this net**: 25–30 damage per turn against 16–32 incoming; Kaiser Crab needs about 14 turns and the bot dies at turn 6; Sandpit kills 31% of Insatiable losses | 15% (153/998) | 79% of fights: no sampled line wins; lookahead +4 pp (ns); potions are already used. NECRO1 had the same deck stats and went from 10% (250M) to 26% (500M) and 43% (1B) act-2 boss wins with training | **more training** (NECRO3 is ahead of NECRO1's curve); search at eval gives ≤ 4 pp |
| 3 | Act-2 hallway and elite fights (not traced here) | 25% (252/998) | — | next diagnosis |
| 4 | **Run policy locked into one deckbuilding script**: never Smith, never remove, never skip | contributes to 1–2, small | Overrides with combat unchanged: Smith at ≥ 70% HP +0.31 floors (CI +0.05 to +0.57); targeted removal −0.59; both −0.05; the net's own removal choice −2.5. NECRO1 also collapsed its run entropy (0.09 at 1B) and still improved | low priority; reward or obs would be the class (no evidence yet that either helps) |
| 5 | Act-3 bosses | 1.5% now | 5% greedy wins on 76 starts; Test Subject 0 of 36; the lane's BOSS_ACT3 pool holds 1 snapshot, so 19% of boss practice replays one deck | turn BOSS_ACT3 practice down until the pool holds ≥ 8 states |

**Recommended next change (NECRO3, at its next resume):** `practice.boss_mix` from
(0.15, 0.20, 0.65) to about (0.50, 0.40, 0.10). Act-1 bosses cause 3× the deaths of act-2
bosses but get a quarter of their practice starts, and the act-3 share mostly falls back to act 2
or one snapshot. Expected effect: about 3× as many act-1 boss practice starts. NECRO1 took act-1
boss wins from 55% (250M) to 68% (500M) with ordinary training, so aim for ≥ 60% act-1 boss wins
on DEV-1000 by 250M (now 47%). That would remove about a quarter of current deaths and raise
act-2 boss entries by roughly 25%. Not tested here: it is a schedule change, and the prediction
comes from NECRO1's curve and the practice counts, not from a run.

## Files

Scratch (not committed), `target/rln36/`: harvests `h-dev`, `h-smoke`, counterfactuals `cf-own`
(act 2, k3 n4 + 16 samples), `cf-a2k4n8`, `cf-act1`, act-3 traces `act3`, override runs `o-*`,
and the checkpoint and lib copies.
