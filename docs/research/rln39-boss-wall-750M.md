# RLN39: why NECRO3 at 750M still loses to act-2 and act-3 bosses

Diagnosis only; no trainer, net or lane change. NECRO3 750M pin (`state-750M.pt`, sha256 `ac5bb103…`),
`ema0.999`, greedy, lane lib build `e6dd5bfdabd8089f`. Tools: RLN36's `rln36_boss.py` /
`rln36_report.py` (one new flag, `harvest --unstick`), `rln39_deaths.py` (§1), `rln39_entry.py` (AUC,
deck), `rln39_trace.py` (Kaiser Crab / Insatiable mechanics, why runs cap), `rln39_overrides.py`
(§4) and the benchmark `rln39_fightbench.py`. The harvest (DEV-2000 + SMOKE 23,000,000–23,001,999,
4,000 greedy runs, 6,889 boss fights) reproduces `necro3-750M-dev1000.jsonl` in 999/1000 run hashes.

## Verdict

**The act-2 bosses are no longer the whole wall; the entry state and the scripted deck are.**
At 100M the net lost act-2 bosses in play. At 750M it wins Knowledge Demon 79%, Kaiser Crab 35%
and The Insatiable 42%, and what is left is mostly fights the net cannot win from the state it
enters them in.

- **Act-2 bosses cause 31% of deaths** (Kaiser 14%, Insatiable 13%, Demon 4%). Of the Kaiser and
  Insatiable fights, 61–63% have a winning line among 64 sampled plays. The other 37–39% enter at
  70% HP with 1.4 potions (wins: 90%, 2.0) and **no sampled line wins** (§2). Only about 22% of the
  fights are lost by the net's own play (a win exists, greedy missed it); search adds +3 pp (ns).
- **Never Smithing is now a large loss.** Forcing Smith at ≥ 70% HP at every rest site lifts the
  mean floor **+1.74** (CI +1.35 to +2.13; +0.31 at 100M), Kaiser 34 → 45%, Insatiable 38 → 50%,
  with the same net and no retraining (§4).
- **One engine/mask gap costs 5.2% of runs.** 52 of 1,000 runs end as `decision_capped` at floor
  ~41 because greedy picks Trial's "Double Down" forever: the option is legal and changes nothing
  (§5). Masking it gives +0.32 floors and 24% more act-3 boss entries.
- **Act 3 is 25% of deaths** and a different problem: 9–15% boss wins, 28% reachable by 16 samples.
- **Training keeps working.** The fight benchmark gains +1.5 pp from 500M to 750M (CI 0.0 to +3.0),
  +3.2 pp on The Insatiable (−0.1 to +6.5), +2.0 on Kaiser Crab (−1.4 to +5.4). Per-250M gains are
  small per fight, which fits +1.5 floors.

| checkpoint (DEV-1000, greedy) | mean floor | act-1 boss won | act-2 boss won | act-3 boss won | capped runs |
|---|---:|---:|---:|---:|---:|
| NECRO3 100M | 21.4 | 47% | 10% | 0/5 | 0 |
| NECRO3 500M | 30.0 | | 44% | | 18 |
| NECRO3 750M | 31.5 | 85% | 50% | 47/324 (14%) | 52 |

(750M act rates from the 4,000-run harvest; 500M act-2 rate from the brief.)

## 1. Where 750M dies (DEV-1000 eval: 947 deaths, 52 capped, 1 win)

| where | deaths | share | 500M share |
|---|---:|---:|---:|
| act-1 boss | 148 | 15.6% | 18.5% |
| act-1 elite / hallway | 19 / 11 | 2.0% / 1.2% | 2.7% / 1.4% |
| act-2 boss | 290 | 30.6% | 29.2% |
| act-2 elite / hallway | 86 / 159 | 9.1% / 16.8% | 12.1% / 15.4% |
| act-3 boss | 138 | 14.6% | 11.9% |
| act-3 elite / hallway | 28 / 66 | 3.0% / 7.0% | 2.8% / 5.9% |
| outside a fight | 2 | 0.2% | |

Top 10 killers: Kaiser Crab 131 (13.8%), The Insatiable 123 (13.0%), Vantom 48 (5.1%), Queen 47
(5.0%), Test Subject 47 (5.0%), Aeonglass 44 (4.6%), Decimillipede (elite) 38 (4.0%), Knowledge
Demon 36 (3.8%), The Kin 27 (2.9%), Entomancer (elite) 25 (2.6%). Next: Infested Prisms 23,
Slumbering Beetle 22, Hunter Killer 22 (2.3–2.4% each). Act-2 elites and hallways are 26% of
deaths, spread over 20+ encounters; none above 4%.

Boss fights in the harvest (4,000 runs, real entries):

| act | boss | fights | win % | entry HP % won / lost | potions won / lost |
|---|---|---:|---:|---|---|
| 1 | Ceremonial Beast / Lagavulin / Soul Fysh / Kin / Vantom / Waterfall | 289–371 each (DEV only) | 90 / 82 / 89 / 85 / 74 / 87 | 84–95 | 1.6–2.0 |
| 2 | Kaiser Crab | 777 | 35 | 93 / 78 | 2.0 / 1.6 |
| 2 | Knowledge Demon | 776 | 77 | 86 / 74 | 1.8 / 1.5 |
| 2 | The Insatiable | 842 | 42 | 89 / 77 | 1.9 / 1.6 |
| 3 | Aeonglass | 225 | 15 | 88 / 75 | 2.1 / 1.6 |
| 3 | Queen | 240 | 12 | 96 / 72 | 2.0 / 1.5 |
| 3 | Test Subject | 228 | 10 | 93 / 75 | 1.9 / 1.7 |

## 2. Kaiser Crab and The Insatiable (≥ 777 real fights each; 200–400 counterfactual fights)

Greedy, per turn (`rln36_report.py play`); 100M numbers from RLN36 §2 in brackets:

| boss | outcome | n | turns | dealt/turn | incoming/turn | block/incoming | turn of death (median) | entry HP % | potions used |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Kaiser Crab | won | 269 | 10.4 (9.2) | 44.4 (48.3) | 26.7 (29.5) | 0.40 (0.23) | | 93 | 1.7 |
| Kaiser Crab | lost | 508 | 7.5 (5.9) | 31.2 (30.4) | 29.7 (31.5) | 0.34 (0.21) | 7 (6) | 78 | 1.6 |
| The Insatiable | won | 354 | 8.5 (8.2) | 42.5 (43.5) | 16.9 (16.9) | 0.37 (0.22) | | 89 | 1.6 |
| The Insatiable | lost | 488 | 8.1 (6.9) | 27.6 (27.1) | 17.2 (16.6) | 0.38 (0.25) | 8 (7) | 77 | 1.5 |

The net now blocks 0.34–0.40 of incoming damage (was 0.21–0.25) and ends 40–50% of turns on its own
(was 0%), so the +30 pp win rate came from defence and tempo; damage per turn in losses is
unchanged (31 / 28). The belt is empty at death in 99% of Kaiser losses; 31 of 488 Insatiable
losses die with a potion.

Kaiser traces (`rln39_trace.py`, 396 greedy fights, reproduced 396/396): wins kill Rocket first
(126 of 130 first arm kills) at turn 7.4 and deal 41.8 arm damage per turn; 193 of 261 losses
kill no arm at all, 68 kill one, none both. The facing flips 0.6–0.7 times per turn in both.

Insatiable traces (386 fights, 386/386 reproduced): **95 of 239 losses (40%) are Sandpit kills**,
the counter at 1 at the last decision with HP + block above the visible incoming (31% at 100M).
Frantic Escapes played: 4.5 in wins, 3.9 in losses (4.2 / 2.8 at 100M). Counter at the last
decision in losses: 1 → 151, 2 → 43, 3 → 30, 4 → 14.

**Is the gap "never finds a winning line" or "no line wins from this state"?** `cf` replays 100
fights per boss and plays each from the same state as greedy and as 64 sampled plays:

| boss | greedy | any of 16 (200 fights) | any of 64 (100 fights) | mean sample |
|---|---:|---:|---:|---:|
| Kaiser Crab | 35% (29–42) | 50% (43–57) | 63% (53–72) | 36.6% |
| The Insatiable | 38% (32–45) | 55% (48–62) | 61% (51–70) | 37.8% |

Entry state of the same 100 fights per boss (Kaiser / Insatiable):

| group | fights | entry HP % | upgrades | potions |
|---|---:|---:|---:|---:|
| greedy wins | 39 / 41 | 90 / 90 | 3.3 / 2.8 | 2.1 / 1.9 |
| greedy loses, a sample wins | 24 / 20 | 88 / 81 | 2.2 / 3.0 | 1.8 / 1.9 |
| no sample wins (0 of 64) | 37 / 39 | **71 / 69** | 2.3 / 3.8 | **1.4 / 1.3** |

So about **60% of the lost Kaiser and Insatiable fights (37 of 61, 39 of 59) have no winning line
for this net from that state**, and those enter at 70% HP and 1.3–1.4 potions. About 40% (24 of 61,
20 of 59) enter like the won fights (88% HP) and are lost by chance or by play: a training or
search problem. At 100M the sampling ceiling was 3% (Kaiser) and 33% (Insatiable); it is now 63% and
61%, so training has moved the "play" part; HP and deck now bound the rest. Search (k = 4 top
actions × 8 reseeded greedy rollouts, 60 fights per boss): Kaiser +1.7 pp (−8.6 to +11.9), Insatiable
+5.0 pp (−6.1 to +15.9), both ns, about +3 pp in all (RLN36: +4 pp).

## 3. Act-3 bosses

Real entries from the harvest (greedy), and `cf` on 40 fights per boss with 16 samples:

| boss | fights | greedy win | HP lost/turn | incoming/turn | turn of death | greedy (cf, 40) | any of 16 |
|---|---:|---:|---:|---:|---:|---:|---:|
| Aeonglass | 225 | 15% | 6.5 | 21.5 | 8 | 12% | 38% |
| Queen | 240 | 12% | 7.1 | 29.2 | 7 | 22% | 35% |
| Test Subject | 228 | 10% | 6.4 | 33.7 | 7 | 2% | 10% |

Wins enter at 88–96% HP, losses at 72–75%; entry HP alone gives AUC 0.59–0.72. Dealt/turn in
losses is 31–37 against 50–58 in the few wins (Aeonglass, Queen); Test Subject wins are not faster
(31). Practice starts (25 min, 27 distinct first-start fights): greedy 1 of 27 (4%), potions-first
3 of 27, any of 8 samples 4 of 27. The lane's `BOSS_ACT3` pool is now 15 snapshots (1 at 100M); its
practice starts are 70,554 against 76,136 for `BOSS_ACT2` and 25,778 for `BOSS_ACT1`, with act-3
practice winning about 8% (37 of 471 ended).

## 4. Deck

Won against lost entries at act-2 bosses (5-fold ridge-logistic AUC; "deck counts" are RLN36's, no HP):

| boss | n | win | AUC HP % | AUC deck counts (100M) | AUC all state (100M) | losses removed at the ≥ 95%-HP win rate |
|---|---:|---:|---:|---:|---:|---:|
| Kaiser Crab | 777 | 35% | 0.68 | 0.68 (0.58) | 0.77 (0.82) | ≤ 21% |
| Knowledge Demon | 776 | 77% | 0.63 | 0.69 (0.58) | 0.73 (0.67) | ≤ 28% |
| The Insatiable | 842 | 42% | 0.64 | 0.67 (0.59) | 0.71 (0.66) | ≤ 21% |

Deck counts explain more than at 100M (+0.09 to +0.11 AUC) but the whole state not more. Won and
lost Kaiser decks are the same size and have the same upgrade count (29.3 vs 29.6 cards, 2.8 vs 2.8
upgrades), the lost ones hold more gold (102 vs 77: unspent). Cards present vs absent, pp: Kaiser
Demesne +21 (85 decks), Defy +14, Squeeze +12, Lethality +11, Neurosurge −14; Insatiable Demesne
+18, Lethality +12, Squash +10, Grave Warden −8; Demon Lethality +16, Neurosurge −16, Flatten −10.
(At 100M no card moved a boss by more than 12 pp.)

Run-level overrides (`harvest --smith 0.7 --remove --target`, combat and every other decision the
net's; DEV-2000 seeds, paired on seed). RLN36 §5 at 100M in the last column:

| variant | overrides | mean floor | act-2 boss won | floor vs base, paired (95%) | at 100M |
|---|---|---:|---|---|---|
| base | | 31.38 | 572/1150 (50%) | | |
| smith at HP ≥ 70% | 5,512 | 33.12 | 739/1270 (58%) | **+1.74 (+1.35 to +2.13)** | +0.31 |
| smith at HP ≥ 40% | 8,285 | 30.06 | 522/1029 (51%) | −1.32 (−1.80 to −0.83) | |
| remove curse/Strike/Defend | 2,796 | 30.41 | 561/1069 (52%) | −0.96 (−1.42 to −0.50) | −0.59 |
| smith 70% + removal | 5,356 + 2,874 | 31.94 | 699/1193 (59%) | +0.57 (+0.08 to +1.06) | −0.05 |

Smith at 70% adds 3 upgrades by the act-2 boss (2.8 → 5.8) and 5 at act 3, enters the boss at lower
HP (76% vs 82%) and still wins more: Kaiser 34 → 45%, Insatiable 38 → 50%, Demon 79 → 79%, act-3
bosses unchanged. Smith at low HP loses floors (healing is what the net learned). Removal still
loses floors: it costs gold that buys cards and potions. **Deck quality now explains more than at
100M, and the upgrade lever is large.**

## 5. Observation and engine

The checkpoint trains with `obs_v2`, `obs_full` and `obs_events` on, so it sees the following.

| mechanic | in the net's tokens | engine vs decomp (`ref/`) |
|---|---|---|
| Kaiser: Surrounded and the facing | player's facing and each arm's "behind" flag (v2 `SIDE_AT`, RLN21); traces show both flip 0.6–0.7 times per turn | `Surrounded` direction update on a targeted card, potion and arm death, ×1.5 from the arm behind: matches |
| Kaiser: arms, cycles, Crab Rage | arm HP, 3-move intent forecast, `CRAB_RAGE`, `BACK_ATTACK_*` in the power list | Crusher 219 / Rocket 209 HP (A10), move tables, Crab Rage +6 Strength and 99 block on the surviving arm: matches |
| Insatiable: Sandpit | the counter as the boss's power amount; Frantic Escape's modified cost in hand | Liquify (4 Sandpit, 6 Frantic Escapes, 3 draw / 3 discard at random), −1 at each enemy turn start, forced kill at 0, Frantic Escape +1 to the counter and +1 to its own cost, the Thrash → Bite → Salivate cycle: matches |

No observation gap found for these two bosses. Two caveats: the Sandpit rule "death at the start of
the enemy turn once the counter is 1 at end of turn" is learned, not shown, and 40% of Insatiable
losses die at that counter; the cost of a Frantic Escape still in the draw or discard pile is the
base cost.

**Engine gap (not fixed): Trial's "Double Down" is a legal no-op.** All 52 capped runs of the 750M
DEV-1000 eval (5.2% of runs, 18 at 500M) end as `decision_capped` at 6,000 decisions, floor 36–46
(mean 40.7), HP 3–88, in the event `TRIAL`, where greedy picks `event_option` slot 1 (action 149)
40 of the last 40 decisions. Slot 1 is Reject on the first page and Double Down on the next. In the
game Double Down only opens the abandon-run popup, so the engine (correctly, `docs/semantics/run.md`
EV2) keeps the same page and the greedy net repeats the action for ever. Sampled training play escapes
by chance, so training wastes decisions and the eval loses the run. Masking an option that picked
twice leaves the state unchanged (`harvest --unstick`, 227 overrides in 2,000 runs): mean floor
+0.32 (CI +0.23 to +0.41), act-3 boss entries 324 → 403, with Smith +2.24 floors and 5 wins in 2,000
runs (1 at base, 3 with Smith alone). This is an `sts2-rl` mask or `sts2-core` event change, not a net
change.

Other facts: "boss HP left" and "dealt/turn" for Waterfall Giant, Aeonglass and Queen wins are not
reliable (enemy HP jumps with phases), as in RLN36.

## 6. Causes, ranked by share of deaths (DEV-1000, 947 deaths)

| # | cause | share | evidence | cheapest fix class |
|---|---|---:|---|---|
| 1 | **Act-2 elite and hallway fights** (not traced beyond the killer list) | 26% (245) | 20+ encounters, none above 4% | deckbuilding (Smith lifts all fights' floors +1.74), then a trace |
| 2 | **Act-2 boss lost from a weak entry state** (no winning line in 64 samples; entry 70% HP, 1.4 potions) | ≈ 17% (60% of 254 Kaiser/Insatiable deaths) | §2 | **deckbuilding**: Smith ≥ 70% HP gives +11–12 pp on both |
| 3 | **Act 3** (boss 14.6%, elite 3.0%, hallway 7.0%) | 25% (232) | boss win 9–15%, 16-sample ceiling 28%; practice pool 15 | more training; the act-3 practice share is as large as act 2's |
| 4 | **Act-2 boss lost in play** (a winning line exists) | ≈ 10% + Demon 4% | 40% of lost fights; search +3 pp (ns) | more training |
| 5 | **Act-1 boss** (Vantom 5.1%) | 15.6% | Vantom 74% win; the others 82–90% | more training |
| 6 | **Trial no-op loop** | 52 runs, not deaths (5% of runs) | §5 | **engine/mask**: +0.32 floors |

Shares 2 and 4 are part of the act-2 boss 30.6% (Kaiser and Insatiable 254 deaths, Demon 36).

**Recommended next change: let the run policy Smith at a rest site when HP ≥ 70% (a scripted rule in
the training env, or the run head forced at that decision), at NECRO3's next resume.** Measured
here on the 750M net, no retraining: +1.74 floors, act-2 boss wins 50 → 58%, Kaiser 34 → 45%,
Insatiable 38 → 50%. Targets after the change, on DEV-2000 with the same net family: mean floor
≥ +1.7 over the unforced run at the same decisions, act-2 boss win ≥ 58%, and on the fight benchmark
(play only, fixed entries, `rln39_fightbench.py`) Kaiser Crab ≥ 38% and Insatiable ≥ 43% (now 34.4%
and 40.0%) so the net also keeps its play. The threshold matters: 40% costs 1.3 floors. Add the
Trial mask whenever the engine lane next touches events (+0.3 floors, no training needed).
Not tested here: whether training on smithed decks adds to the +1.74, or sees more of act 3 (the
override raised act-3 boss entries 324 → 452).

## Fight benchmark

`python/rln39_fightbench.py`: `build` (fixed panel of boss-entry states: run seed, action prefix
and v1 digest, replayed and sha-checked), `run CKPT` (greedy win per boss, Wilson 95%), `compare A B`
(paired per boss, Newcombe). Panel `python/rln39/panel-v1.jsonl.gz`: 1,993 real entries from the
750M harvest of DEV-2000 and SMOKE (never TEST): Kaiser Crab 500, The Insatiable 500, Knowledge
Demon 300, Aeonglass 225, Queen 240, Test Subject 228 (all act-3 entries the harvest holds; no
practice states). It runs in **35 s** (one env per fight, one batch) under the 3 GB / 2-thread caps.

| 750M vs 500M (greedy `ema0.999`) | n | 750M | 500M | paired diff (95%) |
|---|---:|---|---|---|
| Kaiser Crab | 500 | 34.4% (30.4–38.7) | 32.4% (28.4–36.6) | +2.0 pp (−1.4 to +5.4) |
| The Insatiable | 500 | 40.0% (35.8–44.4) | 36.8% (32.7–41.1) | +3.2 pp (−0.1 to +6.5) |
| Knowledge Demon | 300 | 80.3% (75.5–84.4) | 78.3% (73.3–82.6) | +2.0 pp (−1.4 to +5.5) |
| Aeonglass | 225 | 14.7% (10.6–19.9) | 15.6% (11.4–20.9) | −0.9 pp (−4.5 to +2.6) |
| Queen | 240 | 12.1% (8.5–16.8) | 12.9% (9.3–17.8) | −0.8 pp (−4.6 to +2.9) |
| Test Subject | 228 | 9.6% (6.5–14.2) | 8.8% (5.8–13.2) | +0.9 pp (−2.9 to +4.7) |
| all | 1,993 | 35.0% (32.9–37.1) | 33.5% (31.4–35.6) | +1.5 pp (0.0 to +3.0) |

750M scores at least as high as 500M on Kaiser Crab (and on The Insatiable and Demon), as the evals'
50% vs 44% act-2 boss wins say. Limits: the entries are 750M's own (the 500M net plays states it
would not reach); the panel is combat play only, so the Smith lever above is not in it (rebuild a
panel from a `--smith` harvest to measure it); at n = 500 a single boss resolves about ±4 pp, so use
the "all" row or the paired CI for a change of 2 pp.

## Files

Scratch (not committed), `~/spirebot/target/rln39/`: harvests `h-dev`, `h-smoke`, overrides
`h-smith`, `h-smith40`, `h-rtarget`, `h-smith-rtarget`, `h-unstick`, `h-smith-unstick`; traces
`trace-k`, `trace-i`; counterfactuals `cf-a2` (16 samples), `cf-a3` (act 3), `cf-s64` (64 samples),
`cf-look-k`, `cf-look-i`; act-3 practice `act3`; benchmark results `bench750b.json`,
`bench500b.json`. Committed: the scripts above, `python/rln39/panel-v1.jsonl.gz`,
`python/tests/test_rln39.py`.
