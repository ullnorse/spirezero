# Plan: an end-to-end PPO Necrobinder bot at A10 (RLN1, design only)

Status: proposal, 2026-11-08. Nothing has been trained. The owner decides §8 before any long job.

Method credit: spire-mentor's "Instinct" campaign (MIT, `~/projects/spire-mentor`, origin/HEAD:
`docs/plans/2026-11-02-ironclad-e2e-ppo-v0111.md` [SM-PLAN], `docs/experiments/2026-11-03-ironclad-e2e-v111/`
[SM-LOG] = CAMPAIGN_LOG, PREREG, EVAL), itself an AlphaDefect replication. We copy ideas, not code.
Marks: **(m)** measured in RLN1, **(s)** sourced, **(est.)** estimate, **(calc)** arithmetic.

## 0. Summary

- One transformer makes every Necrobinder A10 decision with no search, trained by PPO from random
  init on `RunEnvs` whole runs. The policy reads only the honest observation; a value head also
  reads a new privileged buffer.
- The engine is ready to train on: coverage is 0 missing, all 91 Necrobinder cards played, and 2,000
  A10 runs ended with 0 unsupported/stuck **(m)**. One parity hole: no Necrobinder run has reached the
  A10 second boss under the referee.
- No teacher: our search bot wins 0/200 Necrobinder A10 runs **(m)**, and spire-mentor's own
  learning check shows random-init PPO learns at A10 in minutes **(s)**.
- Box rate: ~8k decisions/s central (4–14k) **(est.)**, about 0.7B a day. That is 1B in ~1.5 days
  and 3B in ~4.5 days of uninterrupted box time.
- Time to a first meaningful A10 read (DEV-2000 at 1B): **~2–2.5 weeks from approval**, of which
  ~8 agent-days are code. Central forecast at 3B: ~20% (range 6–40%) **(est.)**.

## 1. Method: what Instinct does, and what we do

| Item | Instinct [SM-PLAN §3–7, SM-LOG] | Ours | Why |
|---|---|---|---|
| Scope | one net, every combat and run decision, greedy at eval, no search | **copy** | the point of the exercise |
| Inputs | entity tokens: combat context (~49), run context, current act boss(es), pooled pile summaries, run body, map cone, telegraph + 3-step intent forecast; mean ~51–57 tokens | **change**: tokens built on the GPU from our fixed `RunEnvs` bytes (2,996 B), one token per non-empty slot (header, creatures incl. Osty, combat cards with pile, hand, intents, deck, relics, potions, map points, decision options); draw/discard/exhaust pooled into summary tokens; target mean ≤64 | their Python tokenizer cost 0.7–1.7 ms/row and bound workers [SM-LOG 06:10, 11:41]; ours is a gather on the GPU. No forecast getter at first (the next-move intent is in the obs) |
| Vocabulary | one append-only ID table, 4,096 rows; 48-column content prior | **copy** the shared ID table (our `vocab` ids); content prior later | our vocab is already append-only and hashed |
| Honest policy | whitelist of visible state; PRIV absent from the trunk input; F7 bit-identical test | **copy**; our obs already passes `tests/leak.rs` | §5 |
| Privileged value | 1 extra block over [trunk; PRIV]: true top-10 draw, true next moves, this act's rooms, future maps/bosses | **copy**: new `priv` buffer from Rust (§4 WP2), value module only | value may see hidden state (`docs/specs/honesty.md` covers the policy) |
| Action dedupe | identical options merge into one class (cost, enchant, per-copy numbers, flags); teacher probs merge | **copy** in Rust: the mask keeps one index per class, plus a class count per decision | fewer, cleaner choices; entropy over classes |
| Auto-advance | a step with one legal class gets no forward pass and no row (15.7% of combat steps) | **copy**: `RunEnvs` already takes single-option decisions; extend it to single-*class* ones | |
| Policy head | per-option logit: MLP([h_option; h_state; e_kind]) → softmax over merged options | **change**: each of our 355 action indices maps to an entity token (hand slot, target, option, map point, shop slot…) by a static table; logit = MLP([h_entity; h_state; e_kind]) | same idea on our fixed index space |
| Net | 4×256 chosen at B-T0 (6×256 planned), RMSNorm, SwiGLU 704, 4 heads, no positions; ~7.4M with the 1M ID table | **copy 4×256** | measured 1.5× faster than 6×256 here (§3) |
| PPO | decoupled PPO, clip 0.2, IS clamp 2, dual-clip 3; 131,072 decisions/iteration; minibatch ~8,192 in length strata; E=2, then E=1 from 550M (+43% rate, no loss of progress) | **copy** clip/batch/minibatch; plain PPO (synchronous learner, so no staleness, no decoupling); E=2 to 250M, then E=1 | one GPU, one process |
| KL control | probe KL on 8,192 fixed rows; lr ×1.5 if <0.005, ÷1.5 if >0.02, within [0.3, 2]; hard stop >0.06 | **copy** | |
| Returns | γ=1; win +1, +0.05 per boss, +0.002 per floor, no HP term; combat advantages chain into the run (λ_c 0.95, λ_r 0.97) | **copy** | replaces our ±1 + floor/HP shaping in `a10-training.md` |
| Entropy | per type over classes / log(n); 0 during warm start, then 0.01→0.002 (combat), 0.0125→0.006 (run) | **change**: no warm start, so start at the high end with a 100M flat phase, then their decay | |
| EMA weights | 0.996 and 0.999 per iteration; EMA0.999 was best at 2B (57.08% vs raw 42.61%) [SM-LOG Nov 7] | **copy**; evaluate raw and both EMAs | EMA was worth ~+14pp there |
| Teacher | R3 no-search pair; acts with prob 1→0 to 80M; KL to it, λ 1→0 by 500M | **drop** (§3.1) | we have no Necrobinder teacher |
| Practice | reseeded snapshots before elites/bosses, boss share 40%→75% | **defer** to a restart after K3 | first prove the plain loop |
| Eval | greedy, paired seeds, exact McNemar, Newcombe CI, sealed TEST read once; DEV every 250M/1B; K-gates | **copy** (§6) | |
| Ops | systemd lane, watchdog, pinned checkpoints with sha256 manifest, stats.jsonl | **copy** (WP7) | memory rules: systemd-run units, checkpoints under `~/spirebot/target/rl-checkpoints/` |

Their result: DEV EMA0.996 16.9% at 100M, 27.8% at 500M, 43.6% at 750M, 52.3% at 1.7B; EMA0.999
57.08% at 2B, still rising, at ~9.1k decisions/s on four RTX A4000s [SM-LOG].

## 2. Necrobinder readiness in our engine

| Check | Result | Source |
|---|---|---|
| Game version | v0.111.0, the same as Instinct | `docs/PLAN.md`, referee README |
| Coverage, Necrobinder × 4 acts | 544/544, 547/547, 624/624, 590/590, 0 missing | `cargo xtask coverage --all` **(m)** |
| Sign-off matrix 2026-10-27 | Necrobinder cells 200/200 in every act, A0 and A10, 0 divergences; rotating 1,200/1,200 | `history/signoff-2026-10-27.md` (s) |
| A10 second boss | **0** Necrobinder runs reached it in the matrix (Glory greedy/shopper A10: 65/62 reached the Glory boss, 0 the second) | same (s) |
| Necrobinder cards | 91 in the pool; none is in BACKLOG's list of 61 cards no loadout plays | `ref/INDEX.tsv`, `tasks/BACKLOG.md` (m) |
| Osty | in the combat obs as an ally creature (alive flag, HP, powers); dead Osty kept | `crates/sts2-rl/src/obs.rs`, `specs/characters.md` |
| 1,000 A10 runs, random agent (seeds 23,000,000–23,000,999) | 0 wins, floor 3.0/3.5 (median/mean), ends died 1,000, 0 unsupported, 0 stuck, 0 capped, 1 refused | `a10_baseline.py` **(m)** |
| 1,000 A10 runs, random run choices + greedy fights | 0 wins, floor 8.0/8.5, died 1,000, 0 unsupported/stuck/capped, 1 refused | same **(m)** |
| 200 A10 runs, our search bot (Ironclad-tuned defaults), seeds 23,001,000+ | **0/200**, floor 12.0/11 (mean/median); 191 died by floor 17 (55 at the act-1 boss); top killers Phrog Parasite 26, Terror Eel 26, Bygone Effigy 24, Byrdonis 19 | `sts2-diff bot --character Necrobinder` **(m)**, 132 s on 8 jobs |
| Oracle gaps (referee can't check) | 14; Crystal Sphere, Fake Merchant, two treasure orders, Lasting Candy, Wing Charm, Void Form endings, Soul Nexus… | `sts2-diff oracle-gaps` **(m)** |

Random and greedy play die by floor 8, so they don't exercise acts 2–3. Act-jump loadouts and the
matrix are the evidence for those acts.

**Gaps**

| # | Gap | Blocks training? | Fix |
|---|---|---|---|
| G1 | No privileged buffer (draw order, next moves, rooms, future maps/bosses) | **yes** for the value design (it could train without one, worse) | WP2 |
| G2 | No dedupe classes; no single-class auto-advance | **yes** (it changes the action semantics) | WP2 |
| G3 | No per-fight cap (100 turns) or per-run decision cap (only 60,000 engine actions) | **yes** (degenerate stalls) | WP2 |
| G4 | No boss-kill reward events in the step output | **yes** (reward) | WP2 |
| G5 | The A10 second-boss path for Necrobinder was never checked against the referee | no; **blocks reporting** a win rate | add an A10 Glory act-jump loadout that reaches it; `tools/diff_run.sh` 200 seeds |
| G6 | Saturation: combat cards 64, deck 64, creatures 8, powers 12 per creature, combat choice >10 options (`unmapped`) | no, if <0.5% of decisions | count saturations and `unmapped` per DEV read; widen the slot if >0.5% |
| G7 | The 1-in-1,000 `refused` | no | identify it in WP2's smoke |
| G8 | Oracle gaps above | no | report how often the policy hits each one (like their accepted gaps) |

## 3. Teacher

| Option | Evidence | Verdict |
|---|---|---|
| None (pure PPO from random init at A10) | their learning check from random init at A10: mean floor 4.36 → 14.84 in 45 min (7.0M decisions, laptop), 1.96 → 14.58 in 15 min on the box [SM-LOG 12:35, 11:24]; AlphaDefect trained from scratch [SM-PLAN §1.6] | **recommended** |
| A0 → A10 curriculum | the RLB13 A10 run policy *lost* at A0 (892 vs 923 of 1,000, p 0.012): skills don't move freely between ascensions | fallback only if the check fails at A10 |
| Our search bot as teacher | 0/200 Necrobinder A10, 191/200 dead by floor 17 (§2) | **no**: it is worse than what PPO reaches in minutes |
| Short combat-only PPO pre-train | RLB4's combat MLP (0.77 on W1 starts) uses another net and obs; a pre-train would need the new net anyway | no; the e2e net learns combat inside runs |

**3.1 The cheap check that proves it learns (LC, ~15 min GPU, before launch):** random init, no
teacher, Necrobinder A10, the launch config. Pass iff the mean floor of the last 10,000 finished runs
beats the first 10,000 by **≥ +3.0** (their gain was +10.5 to +12.6), combat explained variance > 0.1,
and no non-finite value. On a fail, look for a bug, then try A0 for 15 min as the D1 fallback. LC re-runs at
every restart and hot-swap.

## 4. Compute on this box

Measured with `python/rln1_bench.py` (one bench at a time, box idle, GPU shared with the desktop):

| Measurement | Result (m) |
|---|---|
| Env only, `Sts2RunEnvs` Necrobinder A10, random legal, 64 envs × 1 thread | 167,893 decisions/s, 4,627 runs/s |
| same, 256 × 1 / 1,024 × 8 / 2,048 × 14 threads | 176,398 / 476,231 / 546,540 decisions/s |
| Stand-in net, 96 tokens, bf16, inference at batch 1,024: 4×256 (2.63M) / 6×256 (3.88M) | 49,931 / 33,603 decisions/s |
| PPO update (fwd+bwd+AdamW, minibatch 8,192), per epoch: 4×256 / 6×256 | 14,090 / 9,433 decisions/s |
| Collect loop (2,048 envs, 14 threads: step → copy → forward → masked sample) | 45,390 decisions/s (GPU-bound) |

The env costs ~6 µs a decision and the GPU ~110 µs, so the GPU sets the rate. Random runs are short
(37 decisions); deep runs cost more per step, but even a 10× rise keeps the env under the GPU.

| Rate (decisions/s) | Basis | Per day (24 h) | 1B | 2B | 3B |
|---|---|---|---|---|---|
| 8,850 (proxy) | 4×256, 96 tokens, E=1: collect 22 + pre-pass 20 + update 71 µs (calc) | 0.76B | 1.3 d | 2.6 d | 3.9 d |
| **8,000 central** | proxy × 1.6 for ~60 tokens × 0.6 for desktop use, evals, saves (est.) | 0.69B | **1.4 d** | 2.9 d | **4.3 d** |
| 4,000 low | E=2 or 6×256, heavy desktop GPU use (est.) | 0.35B | 2.9 d | 5.8 d | 8.7 d |
| 14,000 high | ~60 tokens, SDPA/compile, idle desktop (est.) | 1.21B | 0.8 d | 1.7 d | 2.5 d |

- They needed ~1.5–2B decisions to pass 50% on Ironclad [SM-LOG], with a teacher worth an estimated
  0.2–1.0B [SM-PLAN §1.3]. Without one, budget +0.5B (est.).
- Necrobinder is harder: their search champion is 5.6% at A10 against 27–30% for Ironclad
  (RLN1 brief), about −1.9 logits (calc). Our own references: the owner won 12 of 59 finished
  Necrobinder A10 runs on v0.111.0 (20%; 12/137 counting abandoned, from the save history) (m);
  our search bot 0/200 here and 15.0% for Ironclad at A10 (`rl-checkpoints/A10/base`).
- **Forecast (est.)**, their curve shifted −1.0 to −1.9 logits, at half their efficiency:
  1B ~8% (3–20%), 3B ~20% (6–40%). Passing 50% is unlikely inside 3B.
- **Honestly this is weeks:** ~8 agent-days of code (§7), then 4–9 days of box time for 3B, with the
  box unavailable for other CPU or GPU work while the lane runs.

## 5. Honesty gate (our F7)

The policy is π(obs, mask); the value module is V(trunk(obs), priv). The gate is
`python/tests/test_e2e_honesty.py` plus a Rust positive control. It runs at G0, at every pinned
checkpoint and before every reported read.

1. **H1, Rust positive control** (`crates/sts2-rl/tests/leak.rs`, a new case): the existing
   perturbations (draw/discard order, run and monster RNG, event RNG, relic bags) leave the `RunEnvs`
   obs bytes unchanged (existing test) **and change the `priv` bytes** in ≥95% of sampled states, so
   the test can see a leak.
2. **H2, logits bit-identical:** 2,000 states from SMOKE runs, stratified by decision kind (Neow,
   map, event, shop, rest, rewards, combat with Osty alive/dead, act-3 bosses). For each batch,
   compute the policy logits with the real `priv`, with `priv` permuted across rows, with `priv`
   from a blindfolded world (`blindfold_for_search`), and with `priv` filled with NaN.
   `torch.equal` on all four, on CPU fp32 and on CUDA bf16 with deterministic algorithms.
3. **H3, gradient isolation:** ∂(Σ policy logits)/∂priv is exactly zero (autograd), and the policy
   forward's parameter set doesn't include the PRIV embedding.
4. **H4, action replay:** greedy play on 50 SMOKE seeds with `priv` resampled at every step gives
   the same action sequence and outcome as with the real `priv`.

A failing H1–H4 means no win rate is reported. Wins must also replay from seed and actions (honesty.md test 3).

## 6. Panels, seeds, reads and stop/go rules

Seed ranges were checked with `rg` across the repo (crates, python, tools, docs, tasks, learn,
xtask) and `~/spirebot/target/rl-checkpoints`: earlier seed arguments sit in 1,000,000–1,310,000,
8,000,000–8,000,999 and 9,000,000–9,000,199; no seed in the ranges below appears anywhere.

| Panel | Seeds (`RUN<s>`) | Use |
|---|---|---|
| NB-SMOKE | 23,000,000–23,999,999 | benches, smokes, LC, honesty states (RLN1 used 23,000,000–23,001,199 and 23,100,000+) |
| NB-DEV | 24,000,000–24,001,999 | DEV-1000 = first 1,000; DEV-2000 = all; every gate |
| NB-TEST (sealed) | 25,000,000–25,003,999 | one read at the end; count-only output until then |
| NB-BOSS | 26,000,000+ | boss-entry states from DEV-style runs × 4 reseeds (diagnosis, descriptive) |
| Training | 100,000,000–199,999,999 | the student, by iteration and env slot |
| Throwaway | 300,000,000+ | LC and smoke trainers |

**Reads.** Greedy argmax over merged classes, raw + EMA0.996 + EMA0.999, one binary per comparison.
DEV-1000 at 25M, 100M, 250M and every 250M after that; DEV-2000 at every 1B. A DEV-2000 read costs ~2M
decisions, about a minute of GPU (calc). Comparisons are paired on shared seeds (exact McNemar,
Newcombe CI). Never compare across panels. Report the win rate with a Wilson CI, deaths by act and
boss, boss conversion, mean floor, caps and saturations.

**Stop/go rules (pre-registered; written in `PREREG.md` before launch):**

| Gate | When | GO iff | Else |
|---|---|---|---|
| LC | before launch | §3.1 | no launch; fix the bug, then the A0 fallback |
| K1 | 100M | DEV-1000 mean floor ≥ 14 (above the search bot's 12.0), 0 NaN, caps <0.1% | diagnose ≤1 day; relaunch only for a bug |
| K2 | 500M | DEV-1000 reaches the act-2 boss in ≥40% of runs, and mean floor +3 over K1, paired | stop (NO-GO) |
| K3 | 1B | DEV-2000 WR ≥ 3% (EMA best) and > the 500M DEV-1000 read on shared seeds, p < 0.05 | stop; read TEST once |
| K4 | 2B, then each +1B | the last 1B gained ≥ +2pp on DEV-2000 (paired, p < 0.05) | stop; read TEST |
| Budget | at D2's day limit | — | stop; read TEST |

**Final pick:** {the stop checkpoint, the two 1B milestones before it} × {raw, EMA0.996, EMA0.999} on
DEV-2000; default stop-EMA0.999; switch only for ≥0.5pp. Then one TEST read: report the WR, the
paired contrast with the search bot on the same 4,000 seeds, and unpaired references (5.6%
spire-mentor's search, 20% the owner's record).

## 7. Work packages (in order; sizes are agent-days, est.)

| WP | Scope | Done-check | Size |
|---|---|---|---|
| WP1 env | `crates/sts2-rl`: dedupe classes in the mask + class counts; single-class auto-advance; per-fight 100-turn and per-run 6,000-decision caps (= loss); boss-kill and floor events for the reward; saturation and `unmapped` counters; a static action→entity table | unit tests per item; 1,000 SMOKE runs with 0 unsupported/stuck; `cargo xtask gate` | 1.5 |
| WP2 priv | `priv` buffer (true top-10 draw, each monster's next move, this act's rooms incl. `?` outcomes, next act's boss and map), FFI + Python view | H1; leak.rs green | 1.0 |
| WP3 encoder | `python/sts2/e2e/encode.py`: bytes → tokens on the GPU (slot tokens, pooled piles, ID table, numeric stem), ≤64 mean tokens | token stats on 20K SMOKE states (mean, p99, max); bit-stable across batch sizes | 1.5 |
| WP4 net + learner | `python/train_e2e.py`: 4×256 trunk, entity policy head, value module over [trunk; priv]; synchronous collect (2,048 envs, 14 threads) → chained GAE → PPO (E=2 then E=1), KL probe/lr factor, EMA ×2, bf16, resume | unit tests: chained GAE, Σr = env total, class entropy in [0,1], kill and resume reproduce the next 3 iterations; **LC passes** | 2.5 |
| WP5 honesty | `python/tests/test_e2e_honesty.py` (H2–H4) | green on a random-init and an LC checkpoint | 0.5 |
| WP6 eval | `python/e2e_eval.py`: greedy panel runner, sealed TEST mode (counts + sha256 only), paired McNemar/Newcombe, replay of every win; the search-bot arm on NB-DEV/TEST | repeat runs byte-identical; replayed wins = wins | 1.0 |
| WP7 lane | `systemd-run --user` unit (`Restart=on-failure`, `MemoryMax`, `Nice=10`, `CPUWeight` low), watchdog (resume after a crash, stop on >4 starts in 2 h, <30 GB disk, NaN twice in 100M, a rate under half its median for 10 min), `state.pt` every 5M, rolling every 25M (keep 3), pinned at every gate in `~/spirebot/target/rl-checkpoints/RLN2/` with `MANIFEST.tsv` sha256; a `PAUSE` file the owner can touch while gaming; `stats.jsonl` per iteration | kill -9 and resume tested; disk alarm tested | 0.75 |
| WP8 parity | G5 loadout + 200-seed `diff_run.sh` at the A10 second boss | 200/200 | 0.5 |
| WP9 web (later) | only if TEST beats the search bot by ≥5pp and ≥ 5.6%: ONNX/tract export, torch≡tract on 10K decisions (|Δlogit| < 1e-3), an "Instinct"-style no-search advisor in sts2-web-bot behind a flag | parity check; the advisor's picks replay | 1.5 |

Order: WP1 → WP2 → WP3 → WP4 (LC) → WP5 → WP6 → WP7 → launch; WP8 any time before the TEST read.
That's ~8.75 agent-days serially (one agent at a time). Launch is ~1.5–2 weeks after approval (est.).

## 8. Owner decisions

**Approved 2026-11-08: all defaults D1–D7.**

| # | Decision | Recommended default |
|---|---|---|
| D1 | Target ascension | **A10 from the start**; A0 only as the LC fallback |
| D2 | Compute budget | **5 days of box time** for the first lineage (~3B central), with K1–K4 able to stop it sooner; extend 1B at a time only on K4 |
| D3 | Teacher | **none** (pure PPO), proven by LC |
| D4 | Net size | **4×256** (1.5× the 6×256 rate here; spire-mentor also launched 4×256) |
| D5 | Epochs | E=2 to 250M, then **E=1** (their switch gave +43% rate with progress intact) |
| D6 | Practice (boss snapshot replay) | **later**, at a restart after K3 |
| D7 | Box sharing | the lane runs at `Nice=10` with low `CPUWeight`; the owner touches `PAUSE` to free the GPU for games; no other CPU-heavy job while it runs |

## 9. NECRO2: everything on from decision 0 (RLN29)

NECRO1 learned ~600M decisions from card ids alone and got obs v2, the plan column, practice and
two-chain returns only at 575–839M; spire-mentor had all of them from decision 0
(`docs/research/rln20-mentor-code-diff.md`, rows 1–12). NECRO2 is a fresh net with all of
them, plus row 6, a numeric content prior:

- **`card_stats`**: a fixed vector per card (base and upgraded), relic and potion,
  `sts2_rl::content_stats` (ABI 11), from our own content: type, rarity, target, energy and star
  cost (X, unplayable), keywords, tags, the text's numbers (damage, hits, block, draw, energy,
  summon, Osty damage, forge, the common powers, two others) and the upgrade's deltas; one
  linear map added to each card, relic and potion token's ID embedding (`E2ENet` doc).
- **`emb_std` 0.4** (the input embeddings' init std), **`v_warm` 3** (value-only updates first),
  **`ent_flat` 374M** (entropy flat until then).

Launch (not run yet; NECRO2 needs its own unit, and its `status`/`pause`/`resume` take the same
`LANE_OUT` and `LANE_UNIT`):

```sh
LANE_OUT=~/spirebot/target/rl-checkpoints/NECRO2 LANE_UNIT=necro2-e2e \
LANE_ARGS='--set obs_v2=true --set practice={"enabled":true} --set returns="two_chain"
 --set adv_norm="per_type" --set kl_source="batch" --set micro=1024 --set compile=true
 --set card_stats=true --set emb_std=0.4 --set v_warm=3 --set ent_flat=374000000' \
tools/e2e_lane.sh start
```

Practice uses its defaults (NECRO1's later share and mix changes are not carried over).

## 10. NECRO3: the full view (RLN30), event options (RLN33), potion discards (RLN32)

NECRO2 pools the draw, discard and exhaust piles into one summary, sees 4 map rows and has no
enemy move history. `obs_full` (default off; with it off every NECRO1/NECRO2 checkpoint gives the
same bits, `python/e2e_full_identity.py`) adds what a player sees on top of the v2 inputs
(`python/sts2/e2e/encode.py` module doc, `crates/sts2-rl/src/obs_full.rs`):

- **`pile` tokens** (RLN31): one per distinct card (id, upgrade, cost, X, the six flags) among
  the draw, discard and exhaust piles in a fight, with its copies in each pile (symlog) and `cat`
  the piles it is in; the content stats. Sorted by the card's bytes, never in pile order, so the
  draw order stays hidden. The pooled summaries stay.
- **The whole map** (RLN31): one `map_row` token per row ahead that the 4-row window leaves out,
  outside fights in every phase: per column the point's type and its edges (to c-1, c, c+1, and
  the boss) as summed embeddings. Route summaries stay on the window's points and the MAPNODE
  tokens.
- **Enemy move history**: the engine's move log (`MoveLog`, last 3 entries) minus the move shown
  now, so up to 2 past moves per enemy (3 while a forced move holds it), newest first, on the
  enemy's creature token. Each has the move id (hashed), whether its intent was seen, and its
  intent as last shown (kind, damage, hits, whole attack). The intent is the one the observer
  recorded while that move was current; it is never recomputed on the later state.
- `T_MAX` is 128 with the flag on or off. Under NECRO2's pinned policy (197k rows, floors up to
  51) the widest row is 102 tokens (RLN30's encoding: 131) and none overflow.

Launch with all three new inputs (`obs_full`, `obs_events` §10.2, `discard_potion` §10.1; not
run; its own unit; `status`/`pause`/`resume` take the same `LANE_OUT` and `LANE_UNIT`):

```sh
LANE_OUT=~/spirebot/target/rl-checkpoints/NECRO3 LANE_UNIT=necro3-e2e \
LANE_ARGS='--set obs_v2=true --set practice={"enabled":true} --set returns="two_chain"
 --set adv_norm="per_type" --set kl_source="batch" --set micro=1024 --set compile=true
 --set card_stats=true --set emb_std=0.4 --set v_warm=3 --set ent_flat=374000000
 --set obs_full=true --set obs_events=true --set discard_potion=true' \
tools/e2e_lane.sh start
```

**Current restart (2026-11-11 15:17, 501.4M):** the command above with
`--set practice={"enabled":true,"max_uses":16,"min_active":2,"adaptive":true}` (RLN34; `adaptive`
RLN37: practice weights follow where runs die), `--set lr_factor_range=[0.3,2]` and
`--set ent_boost=true` (RLN35). FFI build e6dd5bfdabd8089f. Greedy DEV-1000, paired on 1000 seeds:
250M floor 26.78 (act-1/2/3-first boss wins 72%/36%/14%), 291M 27.03, 500M 30.00
(81%/44%/12%, 2 wins, 18 capped): +3.21 over 250M (CI +2.52..+3.91), +2.97 over 291M
(+2.23..+3.70), −0.41 vs NECRO1 1B K3 (30.41; CI −1.16..+0.33). Earlier: 07:59 (295M)
`lr_factor_range` back to [0.3,2] from [0.3,4] (run entropy rose, run EV fell). Next gate 750M vs 500M.

**750M gate (2026-11-12 00:05, pin at 759.0M):** DEV-1000 ema0.999 greedy: floor 31.50, 1 win, act-2 boss reach 57.7%,
boss wins of entries act1 85% / act2 50% / act3 14% (500M: 81/44/12). Paired: vs NECRO3 500M floor +1.50 (CI +0.82 to
+2.19); vs NECRO1 1B (K3, 1000 shared seeds) +1.09 (CI +0.33 to +1.84). Top killers: Kaiser Crab 65% of reach, Insatiable
62%, act-3 bosses 78–98%. No settings changed. RLN38 GPU bench (one 95k-row batch, mean width 27, 0% MAPNODE rows):
update eager 13.8k / compile-main 6.5k / compile-fixed 7.3k dec/s, collect 74k / 87k / 86k; 0 limit hits. Fixed is +14%
over main (gate was +15%), so RLN38 is not merged. Eager beating compile on update is unexplained: the bench warms only
the first micro shape, so recompiles land in the timed window; re-bench with full warm-up before changing the lane. **Re-bench (00:14, every shape warmed, 30 s):** eager 13.8k / compile-main 15.1k / compile-fixed 15.2k update dec/s (collect 74k / 87k / 87k); the first run's compile numbers were warm-up artefacts. Compile is worth ~9% over eager and the RLN38 fix ~1%: not merged, branch task/RLN38 kept for reference (bench `python/rln38_bench.py`). The update step is at its limit for this net; more speed needs a smaller or cheaper net, not compile work. Each pause/resume costs a value-fit transient (ev_run -10.8 on the first iteration after the 750M pause, back to 0.5 in 4 iterations).

**1B gate (2026-11-12 08:18, pin at 1,000.1M, sha256 `ae09855a…`):** DEV-1000 ema0.999 greedy: floor 34.00, 1 win (replayed), act-2 boss reach 66.9%, boss wins of entries act1 87% / act2 58% / act3 15% (750M: 85/50/13). Paired: vs 750M floor +2.50 (CI +1.76 to +3.24), act-2 reach 66.9% vs 57.7%; vs NECRO1 1B (K3) +3.59 (CI +2.77 to +4.41). Gains per 250M: +3.2, +1.5, +2.5, so no plateau yet; wins stay at 1/1,000. Top killers: Insatiable 121, Kaiser Crab 120, then the act-3 bosses Test Subject 85, Queen 84, Aeonglass 79 (act-3 entries 292 vs 159 at 750M). Eval took 7 min. RLN39 (boss-wall diagnosis at 750M plus a fight benchmark) runs alongside the lane. No settings changed.

**Restart 2026-11-12 08:35 (1,003.6M), user: more boss attempts.** Practice switched from adaptive to a fixed boss-heavy mix: `--set practice={"enabled":true,"max_uses":16,"min_active":2,"adaptive":false,"share":[0.5,0.5],"boss_share":[0.8,0.8],"boss_mix":[0.15,0.4,0.45],"elite_mix":[0.2,0.4,0.4]}` (other flags unchanged). Practice slots 665 -> 1,024 of 2,048, boss share of starts 42% -> 80%: about 3x boss starts per iteration (act-3 boss pool full at 32). First 10 iterations finite; ev_run -22.7 on the first, then 0.07..0.82. Judge at 1.25B.

**RLN39 (merged b7cbe71, `docs/research/rln39-boss-wall-750M.md`):** 750M deaths: act-2 bosses 31%, act-2 elite/hallway 26%, act 3 25%. ~60% of lost Kaiser/Insatiable fights have no winning line in 64 samples (entry 70% HP, 1.4 potions vs 90%, 2.0 in wins): deck/HP, not play. Forced Smith at HP >= 70% +1.74 floors (CI +1.35..+2.13). Trial event: 5.2% of runs end decision_capped looping on a legal no-op (masking it +0.32 floors). Fight benchmark `python/rln39_fightbench.py run CKPT` (1,993 real boss entries, 35 s): 500M 33.5%, 750M 35.0%, 1B 35.9% (1B-750M +0.9 pp, CI -0.5..+2.3): from fixed entry states boss play barely improves; the 1B floor gain came from reaching bosses in better shape.

Measured in one NECRO2 pause (2026-11-10, RLN30):
- **LC** (§3.1; random init, NECRO2's config + `obs_full`, seeds 300M+, 13 min, 3.4M decisions)
  passes. The mean floor went from 2.84 (first 10k runs) to 11.12 (last 10k), +8.27 (RLN29's LC:
  +11.06 in 15 min). Combat EV was 0.85–0.92 and every value was finite. By decisions it is
  NECRO2's curve (9.18 vs 9.39 at 2M, 11.81 vs 11.84 at 3M); by minutes it is slower, because each
  decision costs more.
- RLN30's speed (−38%) and GPU (11.8–12.5 GB) are superseded by RLN31 below.

**Cost (RLN31).** Profile under NECRO2's pinned policy (`python/rln31_rows.py`): `encode` costs
the same on or off (16–18 ms per 2,048 rows); the net's cost follows the token width. RLN30's
tokens per row were 26.8 off vs 46.4 on: piles +13.6 (17 per fight row), the far map +33 per
non-fight row; the collect forward pads every row to the batch's widest (97 vs 39), and the
past moves' 3-slot embedding sum materialised B x T x slots x d. Now: piles 9.4, map rows 1.7
per row (8 per non-fight row), width 37.8 mean, 64 p99, 102 max. The learner, with `obs_full`
only (off stays bit-identical): the collect's forwards run in 3 width-sorted buckets
(`collect_buckets`); micros hold at most `micro * micro_width` (40) tokens instead of `micro`
rows; the slots are summed with `embedding_bag`; `pack` keeps the extras of only the (<= 20)
tokens that have any (15.4 KB per row vs 13.8 off, RLN30's 22.3).

Measured (2026-11-10, NECRO2's config, seeds 300M+, ABAB, 4 iterations each, the median of
iterations 2–4; other workers shared the machine):
- **Speed**: 6.1k decisions/s off (6.13k/6.15k) vs 5.44k/5.52k with `obs_full` (−10%) and
  5.43k/5.41k with NECRO3's command above (`obs_full` + `obs_events` + `discard_potion`).
  Collect ~ off (37–41k at iteration 3); the update 6.9–7.0k vs 8.2–8.4k off.
- **GPU at micro 1024**: 7.3 GB allocated with NECRO3's command at random init (6.3 off; RLN30:
  9.2). On a 131k-row batch of NECRO2-depth rows the update peaks at 8.6 GB vs 9.0 off (RLN30's
  encoding: 12.0), so NECRO3 should stay under NECRO2's 10.1 GB.
- **LC** (random init, NECRO3's command, 8 min, 2.62M decisions): mean floor 2.81 (first 10k
  runs) to 9.68 (last 10k); every value finite, combat EV 0.89–0.94; by decisions 9.05 at 1.97M
  (RLN30 9.18 at 2M, NECRO2 9.39); 5.7k decisions/s sustained.

### 10.1 RLN32: `discard_potion` and `plan_depth` (both default off / 1)

Config flags (`--set discard_potion=true`, `--set plan_depth=2`; fresh runs only). With both at
their defaults a NECRO1/NECRO2 checkpoint gives the same bits (`python/e2e_full_identity.py`
against the code before RLN32): the logits stay 355 wide (`E2ENet.n_actions`) and no new index is
ever set.

- **`discard_potion`** (ABI 13): the run space grows from 355 to 361 indices, appended after
  `PROCEED`: index 355 + s discards the potion in belt slot s (entity kind `discard_potion`, the
  encoder points it at the belt token; no new parameters, `akind` has room). Listed (the env adds
  `RunAction::DiscardPotion`, `run_action::discard_offers`) only when the belt is full and the
  screen is a rewards screen with a potion reward, or a shop with a potion the player can
  afford. The game also allows a discard on any other screen except a card or bundle choice,
  and in a fight's play phase; the env offers none of those. Discarding then lets the claim or the buy happen
  (`crates/sts2-rl/tests/discard.rs`).
- **`plan_depth`** (ABI 13): plays per plan of the observation's plan column, 1 (default,
  `OBS_DEPTH`) or 2 (`SHORT_DEPTH`). Honesty holds at 2 (`tests/leak.rs`).

The NECRO3 command above sets `discard_potion`; `--set plan_depth=2` is optional.

### 10.2 RLN33: `obs_events` (default off)

Event and Neow options reached the net only as hashed text keys. `obs_events` (default off; ABI
14; with it off every NECRO1/NECRO2 checkpoint gives the same bits, `python/e2e_full_identity.py`)
adds what each option's text tells a player (`crates/sts2-rl/src/obs_events.rs`,
`python/sts2/e2e/encode.py` module doc): 32 numbers (known, HP, full heal, max HP, gold, all
gold, cards, curses, card rewards, choices, remove, upgrade, downgrade, transform, enchant and
its amount, "all cards", duplicate, relics, relic rarity, relics lost, potions, potions lost,
potion slots, fight, foe HP, random, choose, more pages, delayed, minigame, leave) and the ids of
the card, curse, relic and potion it gives and the card, relic and potion it takes, plus the
enchantment. Numbers are the shown, resolved values (the event's vars and the relics' pickup
vars), never hidden rolls. A linear map of the numbers, the ID table's rows of the ids (and their
`card_stats` rows) and an enchantment embedding are added to the option token.

Coverage: 194 of 205 options met have a full vector (every option of 55 of 58 events, Neow 29/29).
Gaps (column `known` 0): Tinker Time's 8 riders, The Future of Potions' potion trade, Crystal
Sphere's minigame picks.

The rollout buffers keep the page's 8 option vectors per row and a slot index per token, not a
vector per token (+0.2 GB GPU at NECRO2's settings).

Measured (2026-11-10, NECRO2's config, seeds 300M+):
- **Speed** (ABAB, 4 iterations each, the median of iterations 2–4): 5.8k/6.1k decisions/s off
  vs 5.9k/6.0k on (~0%); the update 8.2k off vs 7.7k/8.0k on (−3%).
- **LC** (random init + `obs_events`, 8 min, 3.15M decisions): the mean floor went from 2.84
  (first 10k runs) to 10.73 (last 10k), +7.89; every value finite (iteration 1's `ev_combat` is
  NaN with the flag off too).

## 11. NECRO4: from scratch, guided by the human prior (RLN42)

NECRO3 at 1B fights well but builds weak decks: Smith at 10.6% of rest visits and no removals.
The first fix, a warm start from NECRO3 1B with a distilled run head, raised Smith to 74% but
dropped the floor to 22.7, because the 1B fighting is fitted to its own beginner decks. NECRO4
therefore starts from random weights. Details are in `docs/research/rln42-necro4.md`.

- **Net:** 6×256 (layers 6), with a run head for decisions outside combat. Step 0 distils the
  whole net except the value module from the RLN41 prior, on 4,000 prior-following runs
  (held-out agreement 0.86–0.99 per kind).
- **Training:** KL to the prior from step 0, 0.1 until 250M, linear to 0.02 at 500M, then held;
  Smith target and shop card get ¼ of it. FFI build c7fe421e39c7abbd (Trial mask), RLN37 adaptive
  practice.
- **Removal:** a shop's removal is asked once per visit, against every other shop action.
- **Behaviour telemetry** (`behaviour=true`) feeds the watchdog unit `necro4-watch`. Its
  thresholds are in `NECRO4/watch.json`; a failure pauses the lane and writes
  `NECRO4/WATCH_ALERT`.
- **Kill rule at 250M:** Smith at HP≥70% below 50%, or the floor more than 3 below NECRO3 250M.

```sh
N4=~/spirebot/target/rl-checkpoints/NECRO4
LANE_OUT=$N4 LANE_UNIT=necro4-e2e LANE_ARGS="<NECRO3's current command, §10> --set layers=6
 --set run_head=true --set init_from=\"$N4/init-distilled.pt\" --set init_weights=\"ema0.999\"
 --set init_reset=[] --set prior={\"flat\":250000000,\"decay_steps\":500000000}
 --set behaviour=true" tools/e2e_lane.sh start
systemd-run --user --unit necro4-watch python/rln42_watch.py watch --out $N4
```

Started 2026-11-12 15:11. It was restarted at 10.49M with `behaviour=true`, resuming at the same
decision count. Until RLN42 merges, the lane and the watchdog run code from the RLN42 worktree.

## 12. NECRO5 (planned, not started): pure PPO, 3.6× faster (RLN44)

NECRO4's human prior let it learn a rest→Smith→cancel loop (stopped at 38.7M); the owner chose
pure PPO with a much faster learner. RLN44 (`docs/research/rln44-fast-ppo.md`) took the same
workload from 4.3k to 15.6k decisions/s (13.7k at 33 tokens per row) with gradients at the
existing compile's noise level, and adds `practice.hp_coef`. NECRO5 is NECRO3's args (§10) with
adaptive practice (`hp_coef` 0.05), 6×256, the run head, behaviour telemetry, E=1 and the RLN44
flags, from scratch with no prior. Its exact command and watch config are in the research note
§7. Not started: an independent review of RLN44 comes first (owner, 2026-11-12).
