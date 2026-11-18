# RLN20: spire-mentor's e2e PPO code against ours, for learning speed

2026-11-09. Read-only. **SM** = spire-mentor at `origin/main` `cea66662`; **py** =
`SM/sts2-pycli/python/ppo/e2e/`; **rs** = `SM/sts2-rs/crates/`. **Ours** = this repo at `d5f1beb`.
NECRO1 numbers come from `~/spirebot/target/rl-checkpoints/NECRO1/stats.jsonl` and `lane.log`
(788M decisions, read 2026-11-09). RLN13 (`rln13-mentor-audit.md`) compared their docs; this
note adds what only the code shows and does not repeat RLN13's rows.

**What NECRO1 ran:** obs v1, `returns=chained`, `adv_norm=global`, fixed KL probe, no practice
until ~575M (first `practice` stats row at iteration 4,390), 4×256, no teacher.

**Where the gap opens.** Early learning per decision is not 10× slower. Their Ironclad learning
check (random init, no teacher, A10, 4×256) reached floor 14.6 at 7.09M decisions (exp
`CAMPAIGN_LOG.md:123`). NECRO1 was at 12.2 at 7.1M (iteration 54), on a harder character. The
gap opens later: NECRO1 sits at floors 15-17 from 13M to 100M and 24-25 at 650-790M. Its
practice boss fights at 788M survive 21/32 (act 1), 29/128 (act 2) and 0/68 (act 3). So the
suspects that matter are the ones that raise combat skill, boss skill above all, per decision.

## Top suspects

| # | Theirs | Ours | Early effect | Cost to adopt |
|---|---|---|---|---|
| 1 | **A scripted combat policy as an input.** Every combat row carries #1081's S_ctx block, including rl2's 32 state and 24 action features (py/contracts.py:238-239, 252-253; every ACT_* section reads them, :606, :653-677; tokenizer arm `"S_ctx"`, py/tokens.py:1018). Action feature 21 is the hand-written expert priority of that move (rs/sts2-mcts/src/rollout_net.rs:1083), and expert.rs has a Necrobinder pass (rs/sts2-mcts/src/expert.rs:1140). Feature 19 is "this attack kills the target" (:1153), 20 is block against unblocked damage (:1123), 18 is the target's incoming damage (:1139). The state features add incoming, unblocked and the lethal flag. These are always on: no fill event gates them. | Card tokens have id, upgrade, pile, cost and flags, with no damage, block, kill or priority column (python/sts2/e2e/encode.py:9-35). Previews, knapsack and killable exist only in obs v2 (encode.py:37-47), off by default (python/train_e2e.py:242) and off in NECRO1 | **high**. At random init with a zero-init head the policy is uniform, but one linear direction on x[21] reproduces the expert's play. The net does not have to learn card values from ids. This is the likeliest single cause of a 10× gap to a first win | Kill, block and incoming: **0 d**, obs v2 (RLN16/17) in the next fresh run. Priority: ~1-2 d. Expose `sts2_rl::resolve::Greedy`'s plan (crates/sts2-diff/src/greedy.rs:3-6) as a per-action column, either "first move of the best plan" or the plan's score. It needs an honesty check, because greedy ends the turn on a copy and a draw on that copy would read the hidden draw order. Our own idea, no copied table |
| 2 | **Practice from decision 0.** `practice: bool = True` (py/learner.py:220): 40% of slots replay the student's own elite and boss snapshots from the start. The schedule runs from 0 (plan §3.3) | `Practice.enabled = False` (train_e2e.py:136). NECRO1 turned it on at ~575M | **high** for the first win. A win needs the act-3 double boss. Practice weights act-3 boss starts at 65% of boss starts once the cell has 8 entries, and each snapshot plays 4 times, so rare late-game states get trained far more often | **0 d** (flag) |
| 3 | **Intents, forecast, telegraph and encounter id at launch** (WP-ENG-1, `--filled`; exp `CAMPAIGN_LOG.md:73`), plus tactical columns: knapsack max damage, killable, HP lost at end of turn (py/tokens.py:20, :972). Each enemy's intent sits on that enemy's own token | v1: intent tokens with no enemy link; the rest is v2 only (as #1) | **high** in multi-enemy and boss fights (RLN13 T1/T2) | **0 d** (obs v2 at a fresh start) |
| 4 | **Two-chain returns and per-type advantage moments** from the start (py/algo.py:22-41, 619-724) | Built (RLN15) but `chained`/`global` by default (train_e2e.py:237-238), and that is how NECRO1 ran | **medium**. Run rows get their credit through the combat λ (RLN13 P2) | **0 d** (flags) |
| 5 | **Gold and reward potions are taken by the engine.** No reward-screen decision exists for them (kinds 48-50 are `reserved:#1069`, py/contracts.py:780-782, 872-874). Card and relic picks are decisions | The policy claims or skips each reward and may proceed (crates/sts2-rl/src/run_action.rs:10-21, `CLAIM_BASE` :59). It must learn from a 0.002-per-floor signal that gold and potions are worth taking | **medium**, unmeasured. Each unclaimed gold or potion costs run strength. These decisions also count toward our decision totals | Check first (~0.25 d): the share of claim/skip/proceed rows and of rewards left unclaimed, from eval action logs. If it is material, auto-claim gold, and possibly potions when a slot is free, in `E2E` options |

## The rest

| # | Theirs | Ours | Effect | Cost |
|---|---|---|---|---|
| 6 | 48-column content prior per vocab id: type, rarity, cost, damage, block, hits, magic, upgrade deltas, keywords, target, draw, energy (py/content_prior.py, `_fill_prior` learner.py:1022). Loaded only when `--filled` names `content-prior`; the campaign did | id embedding only | medium for card picks early; low after ~100M | ~1.5 d (RLN13 T6) |
| 7 | KL control **off below 80M** with or without a teacher: phase U0 by decision count (py/schedules.py:101, 211-226). m = 1, no early stop | fixed probe from iteration 1; m swung 2.0 → 0.59 → 2.0 over the first 80M (NECRO1 it 10-610), and early stop above 0.06 | low-medium: lr off by 0.6-2× early, never 10× | 0 d: `kl_source=batch`, or no control below 80M |
| 8 | ID and other embeddings init N(0, 0.4) (py/net.py:309, 1388) | N(0, 0.02) (python/sts2/e2e/net.py:165-168). Ids start ~5× smaller than the numeric stem's output and shared bias | low-medium: ids need ~30 iterations of Adam steps to matter | trivial, fresh run |
| 9 | Entropy β flat at 0.01/0.0125 until 374M with no teacher (gate open, `entropy_beta` py/schedules.py:191-205), plus a collapse boost per type (:418-458) | flat to 100M, then decaying (train_e2e.py:204, 276-286). NECRO1's normalised entropy is 0.25 by 13M | low | trivial |
| 10 | Combat obs gives each draw, discard and exhaust card its own token, plus enemy powers and intent history (~50 tokens, exp `CAMPAIGN_LOG.md:79`) | piles pooled into summaries (~26 tokens) | low | — |
| 11 | V-warm: the first 3 updates train only the value (py/schedules.py:99) | none | low | trivial |
| 12 | Policy output init U(±0.01) (py/net.py:1244) | zero (net.py:169-170) | none | — |

**Same on both sides** (checked in code): reward +1 / +0.05 per boss / +0.002 per floor, no HP
term; γ = 1, λ 0.95/0.97; clip 0.2, dual clip 3, value 0.5·MSE with no clip (py/algo.py:771-776,
887); entropy over merged classes / log n; AdamW 1e-4 / 3e-4, wd 1e-4, grad clip 1, warm-up 10;
131,072 decisions per iteration, minibatch 8,192, E=2; EMA 0.996/0.999; sampling at temperature 1
(multinomial, learner.py:1253, vs our Gumbel-max); A10; 100-turn fight cap; dedupe into classes;
single-option steps not stored; launch net 4×256 (B-T0 picked it, exp `CAMPAIGN_LOG.md:121`; the
code default is 6×256, py/net.py:303).

## A from-scratch Necrobinder run in their setup, by default

- **Character:** Ironclad A10 is hard-coded (py/factory.py:36-37). A Necrobinder run means editing
  `CHARACTER`. A10 stays.
- **Teacher: none.** `teacher_pair`, `combat_teacher` and `run_teacher` default to "" (learner.py:270),
  so `teacher_types` is all False and λ_KS is 0 on every row. The entropy gate is open from the
  start, so β stays flat until 374M. KL control is still off below 80M (#7) and the first 3
  updates are value-only.
- **Content prior: no by default.** `filled=[]` (learner.py:227) leaves both the prior and
  WP-ENG-1's telegraph, forecast and encounter id at zero. The runbook's documented launch
  declares `["WP-ENG-1","content-prior"]` (exp `RUNBOOK.md:198-205`), and the Ironclad campaign
  used it.
- **Always on, whatever the flags:** practice (learner.py:220), and the rl2 combat features with
  the expert priority for Necrobinder's cards (#1). The tactical columns are on too.
- The 37M first-win claim could not be traced: `origin/main` has no Necrobinder e2e run, config
  or log. Whatever produced it, the code above is what the public pipeline does by default.

## Recommendation

For a fresh Necrobinder run, turn on everything that already exists at once: obs v2, practice from
0, `two_chain` + `per_type`, `kl_source=batch`. Then build #1's greedy-plan column, after an
honesty check. Measure #5 before changing the env.
