# RLB11: a combat value net for the search

A policy+value net trained with PufferLib on the GPU now scores the search's leaves. Data,
checkpoints and curves are in `target/rl-checkpoints/RLB11/` (main checkout).

| | dev 8,000,000–8,000,999 | test 9,000,000–9,000,199 |
|---|---|---|
| RLB9/10 | 813 | 157 |
| value leaves, u525 net | 878 (discordant 57/122) | 168 (15/26) |
| value leaves, committed net | **891** (49/127) | **176** (10/29) |

Every win replays. The deaths are still bosses: dev Test Subject 38, Queen 24, Aeonglass 14.
Runs take about 2.5× the base's wall time (1,832 s vs 740 s for dev on 16 threads): commons
now score three ends per first action with the net, where the base used the turn score
alone. Bosses and elites got about 4× cheaper.

## The net

- **Data.** `sts2-diff collect-starts`: every fight start of the RLB9 bot's runs on train seeds
  1,010,000–1,012,999, as start records (62,655; each rebuilds the run's fight to the same
  observation). The last 150 runs are held out. Elite starts are repeated 3 times and boss
  starts 6 times. Each episode starts with its hidden part resampled (`Envs::blindfold`).
- **Input.** The leak-tested observation, featurised in torch (`python/sts2/combat_net.py`):
  learned embeddings for cards (per pile and per hand slot), monsters, powers (kind and
  amount), intents, relics, potions and the encounter. Then a 1,552 → 512 → 512 MLP with three
  heads: policy, P(win), and the HP fraction kept. `honest/value.rs` mirrors it in Rust
  (parity < 1e-3, `the_builtin_net_matches_pytorch`).
- **Training.** Masked PPO with 4,096 envs, PufferLib 3.0.0's `PufferEnv` and its advantage
  kernel. The win and HP heads are each fit to their own λ-return in probability space; the
  critic is win + ½ HP. The env now also rewrites the static part every step, so the potion
  belt is the current one, as the search sees it. Throughput was 55–110k steps/s, bound by
  the CPU env and the serial PPO update.
- **Curve.** On held-out starts the greedy net wins bosses 80% at 26M steps, 83% at 300M and
  88.9% at 650M; commons 99.3% and elites 97.9%. P(win) is calibrated: 88.8% predicted vs
  88.9% actual on boss starts.

## In the search (`--value builtin --value-leaves --value-hp 300`)

Each first action's three best turn ends (by the turn score) are scored by the net:
10⁶ × (P(win) + 0.3 × HP fraction kept), less 0.5% of a win per potion used. A fight won this
turn scores its own HP plus a 0.5% certainty bonus. The net's HP-kept estimate is capped at
the HP the player has. Without the cap and the won-HP rule, the search stalled for 60 turns
next to a 2-HP enemy in 5 of 142 common fights: the net's HP estimate beat a sure win.

Unbiased starts (48 train seeds; wins, HP lost per fight):

| | commons (142) | elites (238) | bosses (136) | ms per boss fight |
|---|---|---|---|---|
| base (RLB9/10 search) | 141, 6.2 | 237, 9.9 | 129, 35.9 | 18,500 |
| value leaves, u525 net | 142, 3.1 | 236, 7.5 | 127, 30.3 | 4,900 |
| value leaves, final net | | 236, 7.0 | 130, 29.2 | 4,600 |
| + lookahead `l1 k2`, u525 | | 238, 6.0 | 132, 28.4 | 33,400 |
| the net's policy as the playout | | 237, 10.7 | 131, 36.8 | 22,100 |
| + policy prior at the root (1% per nat) | | 235, 12.5 | 125, 38.9 | 5,200 |

Paired whole runs on train seeds 1,000,000+ (each the complete seed prefix):

| variant vs | n | base | variant | discordant base/variant |
|---|---|---|---|---|
| value leaves (u525) vs RLB9 | 1,133 | 941 | 1,011 | 76/146 (p < 0.001) |
| final net vs u525 | 663 | 586 | 592 | 43/49 |
| boss lookahead `l1 k2` vs final net | 357 | 320 | 324 | 14/18 (stopped) |

What mattered: one calibrated scale at the leaves. Value leaves win more runs, keep more HP
in every fight type and are 4× cheaper than the priority playouts. What did not: a stronger
net (83% → 89% on bosses gave +6 runs of 663), the policy as the playout (slower, no better),
and the policy as a root prior (it is weaker than the search's own ranking). RLB10's
lookahead no longer loses runs on the value scale (RLB10: 162 vs 174), but at 2.5× the time
it gains nothing measurable. It stays off.

## Reproduce

```
target/release/sts2-diff collect-starts --first 1010000 --count 3000 --run-policy builtin --out S.jsonl
.venv/bin/python python/rlb11_starts.py S.jsonl --out SHARDS --eval-runs 150 --runs-per-shard 200
.venv/bin/python python/train_combat_puffer.py --shards SHARDS --eval SHARDS/eval --out RUN \
    --total-steps 650000000            # (run 1 resumed with --resume RUN1/last.pt --fresh-opt)
target/release/sts2-diff bot --first 8000000 --count 1000 --run-policy builtin \
    --value builtin --value-leaves --value-hp 300 --out DIR                       # dev
python3 python/rlb11_paired.py BASE/rows.tsv DIR/rows.tsv                          # paired
```

PufferLib 3.0.0 is installed from its sdist with `NO_OCEAN=1`, `--no-build-isolation
--no-deps` and the Python headers on `CPLUS_INCLUDE_PATH`. Its `setup.py` needs
`c_extension_paths = []` added when `NO_OCEAN` is set, and its pin `numpy<2` is ignored
(numpy 2.5.4). Its `pufferl` trainer is not used: it imports pyro, heavyball, neptune and
wandb, and it has no action masks.
