# RLB14: learned whole-fight playouts in boss leaves

**Result:** a boss-fight leaf that is 30% playout by a small learned policy (`--rollout v2
--value-mix 300`) beats RLB12 on paired whole runs: train 943 vs 923 (p 0.038), dev 925 vs 907,
test 187 vs 180; pooled over the three, 123/78 discordant (p 0.002). The default is unchanged
until the orchestrator merges it.

Data and checkpoints: `target/rl-checkpoints/RLB14/` (main checkout). Code: `honest/rollout.rs`
(v2 net), `honest/search.rs` (`ValueUse::mix`, `Playout::Mixed`, `play_out`).

## The gap (30 act-3 boss losses, train seeds 1,000,032–1,000,670)

The fatal fights of 30 lost runs in `RLB13/base.tsv`, replayed from their exact starts
(`fights --seeds --only last --start-policy builtin --start-value builtin`):

| controller | won |
|---|---|
| the bot (beam 48, keep 6, value leaves) | 0 / 30 (the driver reproduces every loss) |
| clairvoyant (one true world) | 17 / 30 |

Clairvoyance rewins only about half, as in RLB12 (49/102), so the deck is not what loses: the
planner does. The playout policies alone, greedy, on 290 act-3 boss starts of the real bot
(seeds 1,022,000+, 2 worlds each; the bot wins 272): priority list 61.7%, RLB7's `rollout.bin`
63.2%, the value net's policy head 75.3%. A linear-policy playout of a whole boss fight costs
under 1 ms; the value head's 3–5 ms.

## The playout policy

- **Linear REINFORCE** on the real bot's act-3 boss starts (recorded, `--starts`), from
  `rollout.bin`: 69.3% held-out greedy (955 starts × 4 worlds, seeds 1,022,000–1,022,999).
- **v2 net** (`--hidden 32`): v1's 24 features plus 28 enemy and player powers (Intangible,
  Artifact, Thorns, Plating, Ritual, Slippery, Reflect, Curl Up, Enrage, Minion, Hardened Shell,
  Poison; Demon Form, Barricade, Corruption, Feel No Pain, Dark Embrace, Juggernaut, Rage,
  Vigor, Inferno, Rupture, Flame Barrier, Buffer, Regen, No Draw, Intangible, Plating), absolute
  HP, enemy HP and block, attackers, the biggest attack, max energy, discard and exhaust sizes;
  a 32-unit hidden layer feeds the per-card rows. Widened from the linear net so it starts
  playing as it does; gradient checked by finite differences.

| net | REINFORCE steps (240 starts × 16 playouts) | held-out greedy act-3 |
|---|---|---|
| `rollout.bin` (RLB7) | – | 63% |
| linear | 325 | 69.3% |
| v2a | +800 | 71.7% |
| **v2b** (`data/rollout-v2.bin`, the A/B's) | +1,500 | **74.0%** (all bosses 84.5%) |
| v2c | +2,000, lr 0.0015 | 76.5% |
| v2d | +2,400, lr 0.001 | 76.3% |
| clone of the bot's moves (5,083 moves, 179 boss fights, top-1 61.5%), then v2 | 1,365 | 71.7% |

Cloning the current search first did not beat starting from RLB7's net. Training is CPU-only:
the net is tiny and the playouts are the cost (about 1.3 s a step on one core), so the GPU does
not help. It plateaus near 76% (v2c, v2d); v2c/v2d were not run in the search.

## The search

`--rollout FILE|v2 --value-mix M`: in boss fights (`--value-mix-in`), each first action's turn
ends are ranked by the value net as before; the best `--value-mix-ends` (1) are played out by
the policy in the same determinized world, and the leaf is `(1 − M/1000) · value + M/1000 ·
playout`, a won playout scoring as a won fight (with its HP term), a lost one 0. With
`--value-mix-turns T` the playout stops after T turns and the net scores where it stopped. Only
the played-out ends compete, so an unplayed end cannot win by having no playout to lose. Leak
test: `the_mixed_search_does_not_read_hidden_state`.

Screen, act-3 boss starts 1,022,000–1,022,299 (290, the same set as RLB13 and VN1; playouts by
v2b):

| variant | won | discordant base/variant | HP left vs base | time |
|---|---|---|---|---|
| base (value leaves) | 272 | – | – | 16.4 s/fight |
| **m300** | **278** | 5/11 | +0.5 | +5% |
| m500 | 275 | 5/8 | −1.1 | +5% |
| m300, 3 ends | 278 | 6/12 | −0.1 | +18% |
| m500, 3 turns then the net | 278 | 6/12 | +1.3 | +10% |
| m1000 (playout only) | 275 | 6/9 | −1.8 | −3% |

Every mix tied or beat the value leaf, and the pure playout leaf matched it: the playouts carry
signal the net lacks, and the two together are best.

## Whole runs

Paired against RLB12 (`RLB13/base.tsv`) on train seeds 1,000,000–1,000,999, `--rollout v2
--value-mix 300` (`a0-m300`):

**n 1,000: base 923, variant 943, discordant 32/52, p 0.038.** Every win replayed. Test Subject
deaths fell 25 → 14 (Queen 18 → 15, Aeonglass 4 → 7). Runs take about 14% longer (59 s vs 52 s
a run, different machine load).

Dev and test, the same command, paired against RLB12's runs (`RLB12/dev-b48k6`, `test-b48k6`),
every win replayed:

| seeds | RLB12 | RLB14 | discordant RLB12/RLB14 | p |
|---|---|---|---|---|
| train 1,000,000 ×1,000 | 923 | **943** | 32/52 | 0.038 |
| dev 8,000,000 ×1,000 | 907 | **925** (92.5%) | 38/56 | 0.079 |
| test 9,000,000 ×200 | 180 | **187** (93.5%) | 8/15 | 0.21 |
| dev + test | | | 46/71 | 0.026 |

Dev deaths: Queen 29 → 24, Test Subject 29 → 27.

## Not done

- **AZ widening.** `az.rs` already widens end-turn edges at `2·√(n+1)` and draws at `√(n+1)`,
  wider than spire-mentor's (about 23 and 4) at the root's visit counts; the tree tied the beam in
  RLB13, so it was not re-screened. Playout leaves in the tree cost 4,000 playouts a decision.
- Elites: the mix is boss-only; the net was trained on act-3 bosses.

## Reproduce

```
D=target/rl-checkpoints/RLB14; B=target/release/sts2-diff
V="--value builtin --value-leaves --value-hp 300"; S=$D/../RLB12/vc-a/starts.jsonl
$B fights --seeds lost30.txt --every 1 --only last --start-policy builtin --start-value builtin $V \
    --variants "h300;c" --out gap.tsv                                                    # gap
$B rollout-train --starts $S --first 1020000 --count 2000 --rl-count 2000 --only act3 \
    --init rl2.bin --hidden 32 --iters 800 --per-start 16 --max-starts 2000 --batch-starts 240 \
    --rl-lr 0.003 --jobs 1 --out v2a.bin                                                 # v2
$B rollout-eval --starts $S --first 1022000 --count 1000 --only act3 --worlds 4 --rollout v2,value
$B fights --starts $S --first 1022000 --count 300 --only act3 $V --rollout v2 \
    --variants "h300;m300;m500;m300 e3;m500 t3;m1000" --out screen1.tsv                  # screen
$B bot --first 1000000 --count 1000 $V --rollout v2 --value-mix 300 --run-policy builtin --out DIR
python3 $D/../RLB13/paired.py $D/../RLB13/base.tsv DIR/progress.tsv
```

## Next

1. Make `--rollout v2 --value-mix 300` the default (or a named bot) after the merge check.
2. Screen v2c (76.5% playouts) in the search, and m500 with 3 turns then the net (best HP on the
   screen); then a mix in elite fights with a net trained on elite starts.
3. More playouts per leaf (sampled, several per end) now that a playout costs under 1 ms; the
   screen's 3-end variant was flat, so try more worlds instead.
4. Train the playout net on the search's own boss positions (not only fight starts), and widen it
   (a monster-identity input): Queen and Test Subject still cause most deaths.
