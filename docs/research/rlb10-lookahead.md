# RLB10: a multi-turn planner, judged on whole runs

The bot is unchanged: RLB9's one-turn beam with a priority playout. Dev reproduces exactly
(813/1000, every win replayed). Data is in `target/rl-checkpoints/RLB10/` (main checkout).

## Loss table (dev 8,000,000–8,000,999, `losses.tsv`)

The 187 lost runs, each fatal fight replayed from its start with 64 worlds, 256 worlds,
clairvoyant and the two-turn lookahead (`l2`). The seed was also re-run 4 times with the run
policy sampled and the full search.

| class | runs | |
|---|---|---|
| (a) search blunder: 256 worlds or clairvoyant wins the start | 81 | as RLB9 |
| (c) weak deck/HP: a sampled re-run wins | 86 | 157 of 187 seeds won by 1 of 4 re-runs |
| (e) no re-run of 4 won | 20 | RLB9 had 3 with 8 re-runs |

By fight (a/c/e): Test Subject 24/23/10, Queen 16/14/2, Aeonglass 9/8/0, Knowledge Demon 10/2/0,
Insatiable 7/4/0, Kaiser Crab 1/9/1.

**Fights picked because they were lost mislead.** Any change wins about a third of them back:
256 worlds wins 48 of 152 boss starts, clairvoyant 51, the lookahead 49–57, and lookahead in
commons too wins 70 of 187. Screen on unbiased starts (`fights --first 1000000 --count 48
--only big`), then judge on paired whole runs.

## The lookahead (`--lookahead M --depth D --beam2 B --lookahead-in boss|big|all`)

Each first action keeps its `M` best turn ends. Each end is planned `D` more turns, in the same
blindfolded world, with the next hand drawn from it, by a beam of `B`. The line is chosen by the
turn scores, and its last end is played out once. `--close G` runs it only among the first
actions within `G` points per world of the one-turn best. `--close-worlds X` instead searches those
first actions again with X× the worlds. The leak gate covers both.

Three scale bugs: each made the lookahead lose HP or runs until it was found.

1. **A max over noisy playouts.** Playing out every one of the `M` ends and taking the best
   favoured first actions that had more ends. Fix: choose by turn score, play out once.
2. **Later-turn wins at `WON` (10¹²).** The search then counted the worlds where it could kill
   by turn 2 and traded any HP for that. It even ended turn 1 with a full hand. A win in the
   enemy's turn (a poison) scored `WON` − potion cost and slipped past a score check. Fix:
   a later win is scored as a playout win plus HP, decided from the end state.
3. **Later-turn deaths at `LOST`.** The search counted the worlds it died in within D turns.
   Fix: a later death is a lost playout less the enemies left.

Unbiased starts (48 train seeds; HP lost per fight):

| | elites (238) | bosses (136) |
|---|---|---|
| one-turn beam (base, 64 boss worlds) | 237 won, 9.9 | 129 won, 35.9 |
| two turns, before fixes 2–3 | 238, 15.2 | 130, 38.8 |
| two turns, all fixes | 237, 9.9 | 129, 34.9 |
| three turns (`l1 d2`), all fixes | 237, **9.3** | 128 (32 worlds), 36.7 |
| three turns before fix 3 | | 123–124, 43 |

Paired whole runs, train 1,000,000+, base 1,654/2,000. Each line is the complete prefix of seeds
the variant finished:

| variant | n | base | variant | discordant base/variant |
|---|---|---|---|---|
| 2 turns, elite+boss, fix 1 only | 294 | 243 | 225 | 41/22 (stopped) |
| 2 turns, boss only, fixes 1–2 | 456 | 382 | 379 | 35/32 |
| 2 turns on close decisions (`--close 300`) | 275 | 227 | 221 | 21/15 |
| 4× worlds on close decisions | 229 | 191 | 186 | 23/18 |
| 3 turns, elite+boss, all fixes, 32 boss worlds | 209 | 174 | 162 | 27/15 (p 0.09) |

**Why deeper planning loses runs: a horizon effect.** Four of the extra deaths in the 3-turn run
are the Waterfall Giant, which base wins 26/26. Killing it at low HP sets off Steam Eruption. The
lookahead sees that death inside its horizon. Ending turns with full hands pushes the death past
the horizon, where the playout's partial credit (up to half a win for enemy HP taken, relative to
the playout's own start) values it far higher. So it stalls while the eruption grows. The
search's scores (`WON`/`LOST` constants, HP and enemy-HP points, the playout's relative
credit) are calibrated for one turn. Each deeper horizon exposes another mismatch between them.

## What it would take

A multi-turn planner needs one value scale for every leaf: an estimate of the win chance and the
HP kept, not the turn heuristics mixed with constants and playout credit. Candidates: average
several playouts per end into a win-rate estimate, or a learned value net trained on the
playouts' outcomes. Then the lookahead and the close-decision gate can be retested on whole
runs. The playout policy (priority list, no blocking) stays the weak link that every value
inherits.

## Profile

`perf` on a lookahead boss fight: about 98% of the time is engine work (`CombatState` clones
13%, malloc/free/memmove about 30%, damage and turn simulation). The search's own bookkeeping
is about 2%. The cost is node count. No waste is removable outside `sts2-core`.

## Reproduce

```
target/release/sts2-diff bot --first 8000000 --count 1000 --run-policy builtin --out DIR   # 813
target/release/sts2-diff fights --seeds LOST --only last --every 1 --start-policy builtin \
    --variants "w64;w256;c;l2" --out FILE                                                    # table
target/release/sts2-diff run-collect --seeds LOST --first 0 --count 0 \
    --policy crates/sts2-diff/data/run-policy.bin --rng R --worlds 16 --big-worlds 16 \
    --boss-worlds 64 --beam 24 --out DIR                                                     # re-runs
target/release/sts2-diff bot --first 1000000 --count 2000 --run-policy builtin \
    --lookahead 1 --depth 2 --boss-worlds 32 --out DIR                                       # 3 turns
```
