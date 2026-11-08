# A10 training

The long-term objective is the ascension-10 win rate over whole runs, one policy per character. A
run is one episode, a win is a victory, and floor and HP are reported and shaped but never traded
against a win. **The current target is a high A0 Ironclad win rate first** (user, 2026-10-27);
A10 waits for an honest A0 result. Combat-only runs are a throughput tool, not the objective. The
reference harness is `python/a10_baseline.py`; the contract's code is
`crates/sts2-rl/src/{run_envs,run_obs,run_action,resolve}.rs` and the binding is
`python/sts2/env.py`.

## Seeds

| set | seeds (each character plays all of them) | use |
|---|---|---|
| train | 1,000,000–1,099,999 | gradient steps |
| dev | 8,000,000–8,000,999 | tuning, model choice |
| test | 9,000,000–9,000,199 | the baseline and every headline number |

Seed `s` is the game seed `RUN<s>`; the character is fixed per env, not `s % 5`. Test is frozen:
changing the harness constants or this table makes the numbers incomparable. The history replay
and the referee are evaluation oracles, never training data. The harness's policy stream follows
the seed base, so a rerun of a mode plays the same draws.

## Observation and action contract

`RunEnvs` writes 2,996 bytes per env at the offsets in `RunLayout`: a 32-byte header (character,
ascension, act, phase, floors, HP/max HP, gold, energy, deck/relic counts, map point, room), then
deck, relics, belt, map, decision sections and, in a fight, the combat observation and its static
part. Content is in the append-only vocab ids; pin `sts2_vocab_hash()`. The header's HP is as of
the fight's start, the combat section's HP is live. Nothing hidden is read (`tests/leak.rs`);
featurise the bytes in numpy/torch on the batch.

The action space is fixed: 355 indices in 6 `u64` words, the 140 combat indices then 215
run-level ones from `RUN_BASE` (ancient/event/map/rewards/rest/shop/potions/deck selection/
bundles/proceed). The mask is `Run::legal_actions` less the actions `Run::accepts` refuses, so it
is not simply the whole list. Pick only set bits: an unset action raises `STS2_ERR_ILLEGAL` and
applies nothing, and a single-option decision is taken inside the env, not as an agent step.
Record two rejections separately: policy choices outside the mask (must be zero; substitute the
first legal index) and `refused`, the env's counter for listed actions it drops because the
engine would refuse them (plus resolver positions where every action was refused). Unmapped
legal actions (past a block's slots) are in `unmapped`; the combat card choice past ten options
is the known case.

## Episode and limits

An episode is `Run::new(character, "RUN<s>", 10, UnlockState::all(), Act1::Random)` until death,
victory, unported content, a position with no accepted action (`stuck`) or 60,000 engine actions
(`capped`). A step is one agent decision on one env; auto and hybrid resolver actions are engine
actions inside it. Evaluation uses a finite, uncycled seed list; episodes are handed out in env
order, so trajectories do not depend on batch size or threads. `done`, `won`, `end` and `floor`
describe the episode that just ended.

**Terminal observation.** The env resets an ended episode before the step returns, so `obs` after
`done` is the next run's (or zero once the list is exhausted). The ended episode's observation is
now kept in `Sts2RunEnvs.final_obs` (the C ABI `sts2_run_envs_final_obs_ptr`, one observation per
env, zero until an episode ends), so terminal HP and floor are available for shaping. There is a
regression test in `crates/sts2-rl/tests/run_envs.rs` and the Python smoke test.

## Reward

Terminal ±1 (`+1` victory, `−1` any other end, as `Sts2RunPufferEnv`), plus a dense potential on
total floor and HP: `r_t = terminal + 0.01·(F_next − F_t) + 0.25·((H_next − H_t)/max_hp)`, with
`F` the header's total floor and `H` the live HP. On the terminal transition `F_next = floor` from the env and `H_next` is the final HP from
`final_obs` (the ended episode's observation). Discount 0.995; these weights are a starting point, not
a contract. Evaluation is always the unshaped win rate, and checkpoints use dev, never test.
`capped`, `stuck` and `unsupported` are losses in training but reported separately so they are
not read as deaths.

## First combat learner (RLB4)

`python/train_combat_ppo.py` trains a masked PPO MLP (256×2) on `Sts2Envs`, reward +1 for a
won fight and −1 for a loss, 64 envs × 128 steps per update. A 60-update run (60 × 128 × 64 ≈ 0.49M decisions)
went from the random policy's **0.329** win rate to **0.770** on the W1-all start evaluation
(512 episodes, greedy argmax), at about 66k training steps/s and 125k eval steps/s on the 4080.
The checkpoint is `target/combat-ppo.pt` (not committed); the trainer checks the build id,
vocabulary hash, observation size and action count.

Caveat: training and evaluation share the W1-all start set (different episode order), so this is
a policy over that fight distribution, not held-out fights.

`python/eval_combat_policy.py` uses the net as the combat controller in `RunEnvs` (agent mode)
with a random run policy: 50 seeds per character gave 0 wins at A10 (mean floor 6.9–7.3) and 0
at A0 (mean floor 8.5–10.5). The learned fights transfer, but random run choices are the
bottleneck; the next step is an **honest** run policy over visible observations
(`docs/specs/honesty.md`), with the combat PPO. Do not behaviour-clone combat from the
clairvoyant `reach`. A10 is still not reachable.

## Honest bot (RLB6)

`sts2-diff bot` (`crates/sts2-diff/src/honest/`) plays A0 Ironclad from visible state only and
passes the action-leak gate (`tests/it/honest.rs`). Headline: **38/200 = 19.0%** on the test
seeds, every win replayed (`sts2-diff bot --first 9000000 --count 200 --out DIR`, about 40 s on
16 threads; rows, summary and win fixtures under `DIR`). Dev (8,000,000–8,000,999): 22.7%.

| change (1,000 dev seeds) | wins |
|---|---|
| PPO as the combat controller | 0 (of 200) |
| search, 8 worlds × beam 24, one-turn score | 59 |
| elites taken above 70% HP, looser card picks | 78 |
| 32 worlds in boss fights | 89 |
| priority-list playouts to the fight's end, bosses / elites and bosses | 142 / 166 |
| playout win weight 15,000 | 175 |
| path by a walk to the boss (expected HP loss and reward per room) | 190 |
| card-pick bar rising with deck size, no deck cap | 227 |

No gain: lower HP weight or free potions in boss fights (with a one-turn score HP is the
survival signal; 47–84), playouts in every fight (167), the combat PPO as the playout policy
(138 vs 157, 5.5× slower), more worlds outside bosses. Events and ancients are valued by text key
and relic id from a table measured on train seeds (`--measure-options`), never by applying an
option during play; an event whose shown damage would leave the player below 30% HP is left.

## Chance-node search and learned playouts (RLB7)

Headline: **50/200 = 25.0%** on test, every win replayed, and 249/1000 on dev
(`sts2-diff bot --first 9000000 --count 200`; 84 s on 16 threads, 6.7 s per run per thread). The
default is RLB6's determinized search with 16/16/64 worlds; `--worlds 8 --big-worlds 8
--boss-worlds 32` is RLB6.

| dev, 1,000 seeds | wins | wall |
|---|---|---|
| RLB6 (8/8/32 worlds) | 227 | 194 s |
| RLB6, 16/16/64 worlds | **249** | 419 s |
| + learned playouts (`--rollout`, REINFORCE weights) | 252 | 478 s |
| chance-node tree in elites/bosses (1,600 sims), RLB6's beam in commons | 236 | 943 s |

Knee, hybrid tree on 200 dev seeds (RLB6: 47): 100/200/400/800/1,600 elite and boss sims won
27/36/39/32/45. Run-level noise is about ±6 wins on 200 seeds and ±13 on 1,000.

- **Tree** (`--controller tree`): one world sampled per simulation, nodes keyed by the visible
  position (identical cards interchangeable, so play orders merge), PUCT with the learned
  policy as prior, progressive widening on outcomes, horizon at the turn's end, RLB6's turn
  score plus a playout at the leaves. Two things were needed to make it work at all: a fixed
  value scale for PUCT (local min/max normalization blew single-playout noise up to the full
  range) and the one-turn horizon (values past an enemy turn are not comparable with RLB6's
  score). It matched RLB6 on paired boss fights only at 2–3× the time, and lost HP in commons.
- **Plans** (`--plans`): RLB6's beam proposes plans, each scored open-loop in fresh shared
  worlds (no strategy fusion). Level with RLB6 on paired fights: strategy fusion costs RLB6
  little.
- **Learned playout** (`honest/rollout.rs`, `sts2-diff rollout-train`): a per-card-key linear
  softmax over visible features. Greedy playout win rate on held-out elite/boss starts
  (`rollout-eval --first 1090000 --count 200 --only big`): priority list 71.9%/33.7%,
  distilled from the search 71.3%/33.4%, then REINFORCE 82.7%/50.4%. The better playout did not
  win more runs (252 vs 249).
- `sts2-diff fights` replays fight starts from RLB6's runs on train seeds with any controller
  (paired; far less noise than whole runs).

## Learned run policy (RLB8)

Headline: **133/200 = 66.5%** on test and **729/1000** on dev, every win replayed, against
RLB7's 50 and 249 with the same search (`sts2-diff bot --first 9000000 --count 200 --run-policy
builtin`; 129 s on 16 threads). The run decisions are a PPO policy over the run observation
(`specs/run-policy.md`): behaviour cloning from RLB7's hand rules, then 165 PPO iterations of
1,024 train runs with the fights at 4/4/16 worlds and beam 12 (34–60k runs/hour, 177,152
training runs, about 6 hours on 16 threads and the 4080). Cheap-search dev rose from 86 (hand
rules) and 85 (BC) to 323/500 and had not flattened. The action-leak gate covers the policy.
Deaths are bosses: floors 17, 33 and 48.

## Evaluation protocol

200 test seeds per character, every character, two reference policies: `random` (the agent plays
fights too, uniform random legal index) and `greedy` (the agent makes run choices, the env's
greedy resolver plays fights). Same batch size and threads for a comparison; unshaped reward; one
deterministic policy stream. Report per character and mode: episodes, win rate, median and mean
floor, decisions per episode, steps/s, resets/s, both rejection counts and the end-reason counts.
The harness writes `target/a10-baseline-64/a10-baseline.{json,csv}` and prints the table
(`python python/a10_baseline.py --seeds 200 --envs 64 --out target/a10-baseline-64`); generated
results stay under `target/` and are not committed. First baseline (200 seeds × 5
characters): 0 wins in 1,000 random-agent runs (mean floor 3.6) and 0 in 1,000 greedy-fight runs
(mean floor 8.8); the first baseline had one `stuck` (`TheFutureOfPotions` with an event potion,
fixed since) and 7 refusals (2 random, 5 greedy), no `unsupported`.

The clairvoyant `reach` gives an A0 positive control (Ironclad `R35W61N88ZUQ`, replayed to
victory), but it is not honest and not a result. The honest target order is A0 first: train and
evaluate the visible run policy at A0, then move to A10. Reachability at A10 is still not shown:
the port's full-run greedy sweep wins 0/1,000 at A10 (act 2 furthest) and 0/1,000 at A0 (act 3
furthest).

## Throughput

Measured through the Python binding, 64 envs, one thread, `taskset -c 6`, load ~2: **124,688
agent decisions/s** (147,956 engine actions/s, 3,373 resets/s) with random fights, and **6,856
decisions/s** (44,631 actions/s, 378 resets/s) with the greedy resolver. A step is a decision,
not an engine action; Python's per-batch cost is why 64 envs is the floor for a trainer.
Throughput is sufficient — a 1M-decision PPO round costs ~8 s (random) or ~146 s (greedy) of env
time — so the limit is sample efficiency: random run-level play reaches floor 3.6 and never a
win, and even greedy fights with random run choices peak at floor 8.8. The next task is the
honest run policy (visible observations) with the existing combat PPO; train and evaluate at A0
first, then A10. The old positive-control need is met only by a clairvoyant search, which is a
diagnostic, not the bot.
