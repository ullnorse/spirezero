# Exercises

One file per day with `TODO`s, one test file per day, reference solutions under `solutions/`.
Each function's docstring is its contract; the tests check it. The pages in `learn/rl/` say which
file is the day's.

```sh
.venv/bin/python -m pytest learn/rl/exercises/week1/test_day1.py   # one day
.venv/bin/python -m pytest learn/rl/exercises/week1                # a week
.venv/bin/python learn/rl/exercises/week1/day1_loop.py             # the day's script
RL_SOLUTIONS=1 .venv/bin/python -m pytest learn/rl/exercises/week2 # the same tests on the solutions
```

The tests need the FFI library and the venv (`learn/rl/setup.sh` makes both); days 8 to 20 need
torch (day 12, 16, 18 and 19 run without a GPU; the capstone wants one). `common/minifight.py` is the browser toys' mini fight in Python; `common/__init__.py` holds
the observation and action offsets. Later days import earlier ones (`week1.day2_observe`), so
finish the days in order, or copy the solution of a day you skip.

| day | file | runs against |
| --- | --- | --- |
| 1 | `week1/day1_loop.py` | real env: a random legal agent, bits of the mask |
| 2 | `week1/day2_observe.py` | real env: header, hand, intents, action index, cards-first rule |
| 3 | `week1/day3_returns.py` | real env: returns, discount, batch episode splitting, HP shaping |
| 4 | `week1/day4_bandit.py` | ε-greedy and UCB on arms, then over strategies on real fights |
| 5 | `week1/day5_value.py` | mini fight: V and Q by rollout; real env: V per encounter |
| 6 | `week2/day6_qlearn.py` | mini fight: tabular Q-learning, three state keys |
| 7 | `week2/day7_td.py` | mini fight: MC against TD(0), n-step; real env: a table over action classes |
| 8 | `week2/day8_nets.py` | real env: features, a torch value net against the table |
| 9 | `week2/day9_reinforce.py` | mini fight: REINFORCE with a masked categorical; it beats random but stalls at a strike-first policy, which the probe state shows |
| 10 | `week2/day10_ppo.py` | real env: GAE, the clipped loss, a PPO run |
| 11 | `week3/day11_experiment.py` | real env: baselines on a fixed seed, log rows, checkpoints, a run with history |
| 12 | `week3/day12_baseline.py` | mini fight: the lethal bit, a critic baseline and an entropy bonus unlock day 9's block |
| 13 | `week3/day13_eval.py` | real env: the evaluation protocol, standard errors, per-encounter comparison |
| 14 | `week3/day14_features.py` | real env: card embeddings per hand slot, a slot-aligned head, the permutation test |
| 15 | `week3/day15_shaping.py` | real env: four reward functions, the telescoping check, PPO with each |
| 16 | `week4/day16_runenv.py` | run env: the header, the indices after 140, run-level rules measured by floor |
| 17 | `week4/day17_runppo.py` | run env: run features, PPO over run-level decisions with a floor reward |
| 18 | `week4/day18_debug.py` | histories: a rule-based diagnosis, a sweep picked by the protocol |
| 19 | `week4/day19_engine.py` | real env: the mask's and buffers' promises audited, illegal actions, steps/s against threads |
| 20 | `week4/day20_capstone.py` | real env: the slot net trained for a time budget, evaluated against the baselines, reported |
