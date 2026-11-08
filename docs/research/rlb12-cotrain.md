# RLB12: the run policy and the value net against each other

Data, checkpoints and curves: `target/rl-checkpoints/RLB12/` (main checkout). Every change was
judged on paired whole runs against RLB11 on train seeds 1,000,000–1,000,662 (base 592/663,
`RLB11/train-vfinal`), chunked, with `python/rlb11_paired.py`.

## Loss table (RLB11 bot, dev 8,000,000–8,000,999: 109 lost)

`loss/losses.tsv`, `python/rlb12_losses.py`. The fatal fight is replayed from its exact start by
the bot's own driver (`fights --start-policy builtin --start-value builtin`; 0 of 109 rewon),
then with 256 boss / 64 elite worlds and clairvoyant. Each seed is also re-run 8 times with the
run policy sampled, and once with clairvoyant fights.

| class | runs |
|---|---|
| (a) search blunder: the fatal start is won by w256 (21) or clairvoyant (51) | 55 |
| (c) weak deck or HP: not (a), but a re-run wins | 54 |
| (e) unwinnable: nothing wins | 0 |

By fight (a/c): Test Subject 23/15 of 38, Queen 12/12 of 24, Aeonglass 9/5 of 14, Knowledge
Demon 3/4, The Insatiable 1/4; 102 of the 109 are bosses, 7 are elites or commons.

- **Losses do not cluster on seeds.** Each sampled re-run wins 63–71% of the lost seeds, and
  108 of 109 are won by at least one of the 8. Losing is fight variance, not bad seeds.
- **Clairvoyance wins only 49 of the 102 lost boss starts.** The planner limits more than hidden
  information, as in RLB9.
- **More worlds on the fatal starts:** w65 rewins 2, w72 7, w128 14, w256 20 of 102. Rewins
  rise with the worlds, but on whole runs the gain was about 1% (below).
- Test Subject traces are damage races lost in forms 2–3 (Nemesis's Intangible turns), some
  entered at low HP. No scoring bug was found.

## Paired whole runs (variant vs RLB11, train seeds)

| variant | n | base | variant | discordant | cost |
|---|---|---|---|---|---|
| **beam 48, keep 6 ends per first action** | 663 | 592 | **618** | 26/52 (p 0.004) | 1.9× |
| boss worlds 256, elite 64 | 413 | 374 | 378 | 20/24 (stopped) | ~3× |
| close decisions (2% of a win) re-searched in 4× worlds | 140 | 127 | 128 | 6/7 (stopped) | 3.4× |
| same, 5% margin | 119 | 108 | 110 | 5/7 (stopped) | 3.4× |
| ei1: value net fit to the search's outcomes | 663 | 592 | 585 | 47/40 | 1× |
| run policy, 40 PPO iterations with the value search | 663 | 592 | 591 | 57/56 | 1× |

**Kept:** a wider turn planner. Searches with value leaves now default to beam 48 and keep 6
(`ValueUse::BEAM`, `ValueUse::DEFAULT.keep`). Without leaves, the beam stays 24.

- **The wider plan helped; more worlds did not.** Two cheaper levers, more boss worlds and
  re-searching close calls in more worlds, gave about +1% at 3× the cost. Planning more turn
  ends per world gave +4%.
- **ei1 (expert iteration).** `sts2-diff value-collect` records the bot's turn-start
  observations with each fight's outcome under its own play: 290k samples from 3,000 runs
  (seeds 1,020,000–1,022,999). `python/rlb12_value_ei.py` fine-tunes the win and HP heads on
  them, with a KL anchor on the policy head. Held-out boss BCE fell from 0.088 to 0.062, and
  calibration was fixed (boss P(win) 0.938 → 0.970 vs 0.976 actual). The RLB11 net
  underestimated the HP the search keeps (0.58 vs 0.66). Runs did not improve: the leaf
  ranking, not calibration, is what plays.
- **Run policy.** The search was 2/2/8 worlds, beam 8, value leaves: 21k runs/h, 84% won
  sampled; at 4/4/16 beam 12 it was 9.5k runs/h, 87%. Training ran 40 iterations × 1,024 runs
  from builtin, at lr 4e-5. Cheap dev went 429 → 429, 421, 434, 428 of 500. Mean deck size went
  30.8 → 31.1, with no other visible change. Flat, as in RLB9: the policy already fits the new
  search.

## Results (beam 48, keep 6)

| | dev 8,000,000–8,000,999 | test 9,000,000–9,000,199 |
|---|---|---|
| RLB11 | 891 | 176 |
| **beam 48, keep 6** | **907** (discordant 52/68) | **180** (10/14) |

Every win replays. Dev wall time is 4,133 s on 16 threads, against RLB11's 1,832 s. The deaths
are still act 3 bosses: on dev, Queen 29, Test Subject 29, Aeonglass 8, The Insatiable 7.

## For RLB13 (AlphaZero-style search)

- The gain came from planning breadth inside a turn, not from more determinized worlds or a
  better-calibrated leaf. A tree that spends its visits on the turn plan, where wider beams
  help, is the right direction.
- The value net's leaf calibration is now measurable on the search's own play
  (`value-collect`), so use those samples as value targets for the new search. A
  better-calibrated leaf alone added no wins.
- Losses are variance over winnable seeds (ceiling ≥ 99%). Judge on paired whole runs, ≥ 600
  pairs: +1% effects need far more than that.

## Reproduce

```
B=target/release/sts2-diff; V="--value builtin --value-leaves --value-hp 300"
$B fights --seeds LOST --every 1 --only last --start-policy builtin --start-value builtin $V \
    --variants "h300;w256 W64;c" --out fatal.tsv                                  # loss table
$B run-collect --seeds LOST --first 0 --count 0 --policy crates/sts2-diff/data/run-policy.bin \
    --rng R $V --out sampledR
python3 python/rlb12_losses.py DIR
$B value-collect --first 1020000 --count 3000 --run-policy builtin --start-value builtin --out VC
.venv/bin/python python/rlb12_value_ei.py --data VC --init RLB11/run2/last.pt --out EI
.venv/bin/python python/train_run_ppo.py --out PPO --init-bin crates/sts2-diff/data/run-policy.bin \
    --meta-from META --lr 4e-5 --lr-end 1e-5 --search 2,2,8,8 --fight-args "$V" --seed-offset 30000
$B bot --first 1000000 --count 664 --run-policy builtin $V --out DIR             # paired, train
$B bot --first 8000000 --count 1000 --run-policy builtin $V --out DIR            # dev
$B bot --first 9000000 --count 200 --run-policy builtin $V --out DIR             # test
```
