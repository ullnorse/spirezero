# RLN13: spire-mentor's merged e2e Instinct against NECRO1

2026-11-09. Read-only audit of spire-mentor at `8aa72765` (code from `592e939c`, docs from
`ccacc7e9`) against our `task/RLN13` base. Every row names both sides by file:line.

Path keys. **SM** = `~/projects/spire-mentor/`; **py** = `SM/sts2-pycli/python/ppo/e2e/`;
**exp** = `SM/docs/experiments/2026-11-03-ironclad-e2e-v111/` (LOG = `CAMPAIGN_LOG.md`);
**plan** = `SM/docs/plans/2026-11-02-ironclad-e2e-ppo-v0111.md`; **ours** = this repo.
NECRO1 numbers come from `~/spirebot/target/rl-checkpoints/NECRO1/stats.jsonl` (2,638 iterations,
345.6M decisions, read 2026-11-09).

**When it can land:** **(a)** now, as a speed or ops change on resume; **(b)** at the K3
restart (1B), which resumes the weights; **(c)** only in a fresh run (obs or net shape changes).

**One enabling idea first.** Their net zero-inits every reserved input (stem weights of reserved
numeric columns, embedding rows of reserved kinds and ids). Filling one later leaves every output
unchanged until training moves those weights (py/net.py:34-48, the "identity rule"). We have
free numeric columns: `NUM` = 48 (ours python/sts2/e2e/encode.py:47), and a non-STATE token fills
at most 11 of them (encode.py:637). If a new input goes into a free column, and the resume zeroes
that column's first-layer stem weights (python/sts2/e2e/net.py:68-75, `stem.a`), the new input can
land at the K3 restart (b) instead of a fresh run (c). The obs rows below use that rule.

## Differences

### Tokens and obs

| # | Theirs | Ours | Their evidence | Value for us | Cost | When |
|---|---|---|---|---|---|---|
| T1 | Intents sit on their enemy's token: the full telegraph of the current move plus a 3-step forecast (kind, certainty, amount, hits) from `peek_intents(3)`, read honestly with no RNG (exp/SCHEMA.md:83, 371; plan:203) | Intent tokens have no enemy link (encode.py:37-38); the obs writes intents in enemy order with no slot (crates/sts2-rl/src/obs.rs:24-25); no forecast in the policy obs, though `priv` already peeks one move ahead (priv_obs.rs:14-16) | none (no ablation); live at launch (LOG:36) | **high**: in multi-enemy fights the policy cannot tell which enemy attacks for how much | link: set the intent token's `owner` to its enemy (an existing embedding, net.py:86) plus 1 obs byte. Forecast: a Rust honest peek (~0.5 d) into free columns | (b): the link changes outputs but no shapes; forecast via zero-init columns |
| T2 | Hand cards carry `card_damage_preview`/`card_block_preview` (plan:202; py/tokens.py:1820); tactical columns: per enemy the knapsack max damage of the hand and a killable flag, and the HP lost if the turn ended now (py/tactical.py, SM/sts2-rs/crates/sts2-e2e/src/tactical.rs:1-4; py/tokens.py:978-986) | Card bytes are kind, upgrade, pile, cost, flags; no damage or block numbers (obs.rs:18-22) | none; their audit found the Rampage preview mattered enough to log as a bug (LOG:82) | **med-high**: Necrobinder damage depends on Osty and powers; the net has to learn the arithmetic from scratch | Rust: per-card preview i16×2 (~0.5 d); tactical in the GPU encoder (~0.5 d) | (b) via zero-init columns |
| T3 | Map: up to 40 `MAPNODE` tokens (the reachable cone), and each move option carries 6 route summaries to the boss (elite min/max, rest min/max, shop max, `?` max) plus fights-to-rest and rows left (exp/SCHEMA.md:356-364; py/tokens.py:1183-1239) | `MAP_ROWS` = 4 rows ahead as tokens (encode.py:51-52, 274); no route summaries, though the obs has the act's whole map with child bitmasks (run_obs.rs:22-24) | none ([AD] cited) | med: map choices decide elites and rests, and act 1 is where we die | GPU encoder only (paths over the 96-point map), ~0.5 d | (b) via zero-init columns on the map tokens |
| T4 | Event options carry honest effect previews (HP, gold, card deltas), with relic-rolling options masked (exp/SCHEMA.md:94, 1113-1120) | An event option is a 16-bit hash of its text key (run_obs.rs:27-28) | none | low-med: events were 53 of 5,329 decisions in their walk (exp/SCHEMA.md:1101-1103) | Rust preview with a reseed honesty test, ~1 d | (b) via zero-init columns |
| T5 | Enemy intent history tokens (`EHIST`, from a per-fight intent log) (exp/SCHEMA.md:84; SM/sts2-rs/crates/sts2-e2e/src/intent_log.rs) | none | none | low: a forecast (T1) carries most of it | Rust log, ~0.5 d | (c): a new token kind grows the `kind` table (net.py:84) |
| T6 | 48-column content prior per vocab id (type, rarity, cost, base numbers, keywords), gathered by id into the stem (plan:240; py/content_prior.py) | ID embedding only (net.py:83) | none | low: by 345M every Necrobinder card has been seen many times | extractor from `ref/`, ~1.5 d | (b) via zero-init columns |
| T7 | Five summaries (draw, discard, exhaust, deck, relics) (exp/SCHEMA.md:53) | four, no relic pool (encode.py:12-14, net.py:139-144) | none | low | small | (c) (a new fixed position) |

### Net

| # | Theirs | Ours | Their evidence | Value for us | Cost | When |
|---|---|---|---|---|---|---|
| N1 | Two policy MLPs, combat and OOC, picked by decision type (py/net.py:57-63) | one head with action-kind and sub-kind embeddings (net.py:149-166) | none | low | small | (c) |
| N2 | A "valve": stop-grad from the value module into the trunk, to flip at a checkpoint if a type's entropy collapses while its value loss rises (py/net.py:72-77; plan:257) | value gradients always train the trunk (net.py:173-195) | none (never flipped in their LOG) | low: our EV is 0.90/0.87 combat/run | a flag | (b) |

### PPO and the loss

| # | Theirs | Ours | Their evidence | Value for us | Cost | When |
|---|---|---|---|---|---|---|
| P1 | The KL probe is 24 random strata (8,192 rows) **of the current iteration**, redrawn each iteration (py/algo.py:1115-1118), on student-acted rows (algo.py:1048-1073) | a **fixed** probe: the first iteration's first 8,192 rows (train_e2e.py:408-409, 566-569), random-policy act-1 states | their probe_kl tracked 0.005-0.012 at E=1 (LOG:145) and fed the R-DDR rule (exp/PREREG.md:140) | **high**: on NECRO1 the factor m is at its 2.0 cap in every iteration since 2,384 (of 2,638); KL 0.002-0.008 on states the policy no longer visits (mean floor 23 vs ~3 in the probe). The control has stopped working | ~20 lines: score the probe on 8,192 rows of the batch just collected | log the batch KL now (a); switch the controller at K3 (b) |
| P2 | Two-chain returns. Combat chain: λ_c only while the successor is a combat row, so a fight's chain stops at the fight's end. Run chain: GAE over the run rows alone, δ_i = R(i→j) + V(j) − V(i) with λ_r = 0.97 between run decisions, R summing every reward in between (py/algo.py:22-31, 304) | one chain over all rows with a per-row λ (train_e2e.py:165-180): a card pick before a 50-decision fight reaches the fight's end only through 0.97·0.95^49 ≈ 0.08 of the combat advantages | none (designed in; T1-T4 unit tests, plan:278) | **high**: run decisions (17% of rows) get their credit through the combat λ. Theirs credits a card pick with the next ~30 run decisions | ~1 d; GAE on the batch, plus the hold-back below | (b) |
| P3 | Run rows are held back until their run ends or for 4 iterations, then train with the chain cut there (py/algo.py:34-41, 478) | each env's 64-step segment is cut and bootstrapped (train_e2e.py:464-466) | none; the cost was a review bug about restart leaks (LOG:38) | med, with P2: a 64-step cut leaves late run rows with only a few run successors | ~1 d with P2; on resume, drop held rows (theirs: LOG:42) | (b) |
| P4 | Advantages are normalised **per decision type** (17 live types in their walk: combat turn, combat choice, card reward, map, event, shop, rest...) by exponentially weighted moments (decay 5e-4 per row) of student rows; a type with fewer than 64 rows uses its family's pool (py/algo.py:619-724; exp/SCHEMA.md:1101-1105) | batch mean and std per family, combat or run (train_e2e.py:571-576) | none | med: in our batch a rare type (a boss relic, a remove) is scaled by map- and reward-dominated run moments | ~0.5 d; needs a decision-type byte per row (the phase is in the obs header) | (b) |
| P5 | Entropy collapse rule: a type whose raw entropy stays under 0.1 nats for 20 iterations gets β ×1.5, at most twice (py/schedules.py:418-458) | none; entropy is per family only (train_e2e.py:183-192, 612) | none (no fire in LOG) | low: our normalised entropy is 0.16/0.20 | small | (b) |
| P6 | lr anneal at the end: ×(1 − 0.7·t) over the last 24 h before the stop, and the KL factor frozen at its start (py/schedules.py:84-98, 145-149, 400-404) | no anneal: lr = base·m·warm-up (train_e2e.py:577-579) | none (their anneal starts Nov 11, LOG:144) | low-med: it lowers the noise of the final pick; EMA0.999 does part of it | ~0.25 d | (b), before the budget's last day |
| P7 | R-DDR plateau remedy: if two 250M reads each gain < +1pp while probe KL does not fall, double the samples per update (262,144) at an unchanged lr and minibatch (exp/PREREG.md:140; Beukman et al. 2026) | none | it never fired (LOG:155-161) | med later: only once WR moves. Needs P1's batch-sampled KL to read | a rule plus `steps` 64 → 128 | (b) |
| P8 | Decoupled PPO with IS clamp 2 for one-iteration staleness (py/algo.py:915-929) | plain PPO, synchronous (train_e2e.py:605-610) | n/a | none: we have no staleness | — | — |

### Reward and schedule

Same reward (+1 win, +0.05 per boss, +0.002 per floor; py/algo.py, exp/PREREG.md:107-109 vs
train_e2e.py:98-102, 451). E=1: theirs at 550M (exp/PREREG.md:156; no loss: 885M read 41.3%
vs 30.9% before the switch, LOG:152), ours at 250M (train_e2e.py:581; NECRO1 iteration ~1,910).
No difference to take.

### Practice (one line)

Their final learner runs practice **by default** (`practice: bool = True`, py/learner.py:202),
and the logged G1 args don't turn it off (LOG:127), so it most likely ran from launch, not "at a
restart" as our plan row says (ours docs/plans/necro-e2e.md:44). The mechanics match the row
(py/practice.py:1-30); RLN12 owns it.

### Eval

| # | Theirs | Ours | Their evidence | Value for us | Cost | When |
|---|---|---|---|---|---|---|
| E1 | Every A10 number comes with an **A9-est**: the A9 rate estimated from the same A10 runs (exp/PREREG.md:7), e.g. 500M 27.8% → A9-est 50.9% (LOG:142) | WR, floor, act-2 boss reach, killers (python/e2e_eval.py:221-222, 601-643) | descriptive only | med: NECRO1's WR is 0 at 345M; a run that beats the act-3 first boss is signal 1 boss earlier | ~0.25 d on existing result files (`kf` already lists boss kills) | (a) |
| E2 | `DEV_JUMP` audit on a big read-to-read gain: replay every run to its fingerprint, then card and relic shares of the decks (LOG:142, 146, 157) | wins replay (e2e_eval.py:512-558); no deck shares | it caught a strategy shift, not an exploit (LOG:146) | low-med: a cheap guard against an engine exploit in a new strategy | ~0.5 d | (a) |
| E3 | Z5: share of DEV-2000 seeds where raw/EMA0.996/EMA0.999 disagree (exp/PREREG.md:141): 54.2% at 2B (LOG:167) | three variants, compared pairwise | descriptive | low | small | (a) |
| E4 | DEVBOSS: 200 boss-entry states per cell × 4 reseeds, student vs MCTS 1000/4000 (exp/EVAL.md:81-120) | NB-BOSS panel planned, descriptive (docs/plans/necro-e2e.md, §6) | not read yet | same as planned | — | — |
| E5 | Web/mod serving: a hand-written Rust forward over exported weights with a bit-equal tokenizer port and parity fixtures (SM/sts2-rs/crates/sts2-e2e/src/lib.rs:1-10, tokens.rs:1-3) | WP9 plans ONNX/tract (docs/plans/necro-e2e.md, §7 WP9) | shipped in their workbench and mod | low now; WP9 only | — | after TEST |

### Ops

| # | Theirs | Ours | Their evidence | Value for us | Cost | When |
|---|---|---|---|---|---|---|
| O1 | Runtime check that members of a merged class tokenize identically; it fails closed and logs a `worker_error` record with the failed decision (py/tokens.py:1086; py/actionlog.py:48-49). It found two merge-key bugs after 1.1B: a per-copy `enchantment_used`, and a combat choice key without its **pile** (hand vs draw pile) (LOG:160, 163) | no member check. Our combat-choice key is `same_card` with no pile (crates/sts2-rl/src/classes.rs:75-102, 119-123) | 2 real bugs in ~1 of 2,000 segments (LOG:160) | med: the same pile merge may hit our choose-from-pile cards | a counter in the Rust dedupe (members' obs slots equal); check whether any Necrobinder `ChooseCards` mixes piles; ~0.5 d | (a) as a counter; a key fix changes classes, so (b) |
| O2 | A resume moves stats rows past the resumed iteration to `stats.orphan.jsonl` (LOG:95) | a resume appends again (train_e2e.py:718-719), so iterations after the last `state.pt` repeat in `stats.jsonl` | n/a | low: the slow-rate watchdog reads a median | small | (a) |
| O3 | THP direct compaction on ≥4 MiB numpy temporaries cost ~40M decisions; fixed with `defrag=defer` and block copies (LOG:154) | we copy 6 MB of obs per step (train_e2e.py:367); this box has THP `enabled=madvise`, so plain numpy buffers aren't hugepages | n/a | none here | — | — |
| O4 | A learning re-check and F7 before every hot-swap (LOG:128, 133, 144, 168) | honesty at every pin (python/e2e_lane.py:24-25); LC at restarts per plan §3.1 | every swap passed | same | — | — |

## Ranked shortlist

1. **P1, the KL probe from the current batch** (*RLN14: KL probe from the batch*). Log the
   batch KL beside the fixed one now (a); switch the lr controller at K3 (b). NECRO1's m has sat
   at its cap for 250 iterations.
2. **P2 + P3, two-chain returns with run-row hold-back** (*RLN15: two-chain GAE and held run
   rows*). (b).
3. **T1, intents on their enemy, plus a 3-step forecast** (*RLN16: intents on their enemy, with a
   forecast*). (b) with the zero-init rule.
4. **T2, damage and block preview plus tactical columns** (*RLN17: damage preview and tactical
   columns*). (b) with the zero-init rule.
5. **P4, per-decision-type advantage normalisation** (*RLN18: advantage moments per decision
   type*). (b); it can share RLN15's restart.
6. **E1 + O1, A9-est in e2e_eval and a class-member check** (*RLN19: A9-est and the class-member
   counter*). (a).

Next in line: T3 (route summaries, (b)), P7 (R-DDR, once WR moves), P6 (final anneal).
