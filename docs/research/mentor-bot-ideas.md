# spire-mentor: ideas for the honest bot

Source: `../spire-mentor` (a sibling checkout of github.com/tfriedel/spire-mentor) (MIT). Paths below are relative to that repo; `SM`
means `sts2-rs/crates/sts2-mcts/src`, `RA` means `sts2-rs/crates/sts2-runai/src`. These are ideas
only: no code is copied, and quotes are short evidence.

## 1. Honesty verdict

**Combat search: honest by our definition (the deployed mode). Run policy: mostly honest, with one
leak, in the event observation. Their 90% is honest for combat, and partly clairvoyant only for
RNG-dependent event outcomes.**

- Their search has four `HiddenInfoMode`s (`SM/lib.rs:852-948`). `Clairvoyant` is the engine
  default ("the root sees the true materialized draw-pile order and true RNG cursors", `:853`), kept
  so fixtures stay stable. The **deployed** mode is `BeliefDraw`: the run AI sets it through
  `mcts_config_for` and `apply_mode_dialect` (`RA/policy/mod.rs:1239-1262`), and so do the
  workbench and advisor (`sts2-wire/src/emit/mcts.rs:89-92`, comment "The DEPLOYED honest search,
  not the leaky one").
- What `BeliefDraw` does at the root (`sts2-core/src/blindfold.rs:151-155`, `SM/lib.rs:1588-1605`):
  it **sorts** the unknown middle of the draw pile into uid order, so no permutation survives. It
  sets `deferred_draw_order` so each draw picks a random card at draw time. It **reseeds every
  combat RNG stream** and the legacy RNG from the search seed, "abandoning the incoming cursors
  entirely" (`blindfold.rs:101-119`). Pinned top/bottom cards stay, and those are visible.
- Chance nodes (`SM/lib.rs:899-946`): any edge that moves an RNG counter becomes a chance node.
  Under BeliefDraw, every draw from an unknown segment of 2+ cards reads the shuffle stream, so
  "every uncertain draw becomes a chance node at every depth". Outcomes are realized by reseeding
  the streams from `(base, n)` and re-applying the edge, so enemy move rolls, random targets and
  draws are all sampled, never read. Outcomes are deduped by a hash of what is observable
  (`chance_outcome_hash`), so duplicate cards do not fan out. Widening is double progressive
  widening (DPW), with separate `(C, α)` constants for draw nodes and EndTurn nodes
  (`SM/lib.rs:1038-1043`, `1160-1178`).
- **How they measured honesty**: `sts2-rs/docs/experiments/2026-07-12-clairvoyance-rent-reanchor.md`.
  Clairvoyant vs honest on the same binary over 2000 paired seeds: ranker 38.35% vs 15.55%
  (−22.8pp), PPO 81.25% vs 55.75% (−25.5pp). Most of the rent was lost in acts 2 and 3. Every win
  rate from before 2026-07-12 is clairvoyant (`docs/ppo-champion-training-playbook.md:109-130`).
  The 89.5% A0 release number (`docs/experiments/2026-10-11-release-winrates.md`) dates from
  October, after the switch, using 8,000 sims per decision in one tree.
- **The leak: event outcomes.** The PPO run observation fills each event option's
  `gold/hp/max_hp/relics/potions/cards` deltas from `preview_option_effect`
  (`RA/ppo/policy.rs:571-595`). That function **resolves the option on a clone of the run state**
  (`sts2-content/src/events/dispatch.rs:273-286`: "the preview advances only the clone's RNG, so a
  later real resolution ... yields the same deltas"). The encoder feeds these values to the net
  (`sts2-pycli/python/ppo/encoder.py:17,123-156`). Under honesty.md this is "the outcome an event
  option rolls", which counts as hidden. For a fixed-cost option (pay 50 gold, lose 7 HP) the delta
  equals the text, so nothing leaks. For a random one (random gold, a random relic or card) the
  number is clairvoyant. The lethal-event veto (`RA/policy/mod.rs:2179-2240`) uses the same
  preview. Its effect on the overall win rate is probably small, but it is real.
- They did not run an action-leak test like our gate 2. Honesty in the combat search follows from
  how it is built (it never reads the cursors or the pile order), not from a check of its outputs.

## 2. Combat search

| Piece | Theirs | Ours (RLB6) |
|---|---|---|
| Tree | One MCTS tree per decision, one node per atomic action (play, target, potion, EndTurn); no reuse across decisions under BeliefDraw (`sts2-bench/examples/combat_asc_eval.rs:353,385`) | K=8 determinized worlds × beam 24 over this turn's plays |
| Hidden info | Belief root + chance nodes with DPW (above) | `blindfold_for_search`, K sampled worlds |
| Horizon | **Playout to the end of the fight**, `COMBAT_ROLLOUT_DEPTH = 200` (`RA/policy/mod.rs:161`) | **One turn**, scored at the start of the next turn |
| Playout policy | Learned per-character "playout net" (below), not uniform/heuristic | none |
| Leaf/terminal value | Win: `0.5 + evaluate_victory` in [1, 1.5], made of HP, potions held and round count (`sts2-core/src/lib.rs:11591-11598`). Loss: −1 + a graded "loss quality" in [−1.45, −1.05) (`SM/lib.rs:965-993`, `RichBoth`) | Hand-built turn score (HP×100, kill 1200, powers…) |
| Exploration | PUCT with a uniform prior; c_puct by budget: 0.8 below 500 sims, 1.41 below 1000, then 2.5 (`SM/lib.rs:1028-1034`) | — |
| Root pick | Regularized π ∝ exp(Q/T), T=0.1 (`SM/lib.rs:212-223`, `3682`) | Argmax mean over worlds |
| Budget | Training and gates: 1000 sims per decision, 4000 at a boss ("eval-cheap", `RA/policy/mod.rs:314-327`). Release: 8000. 1000 sims ≈ 14 ms on one thread (README) | 8 worlds |
| Potions | Ordinary actions in the tree. Optional root opportunity cost (`potion_charge_q`, `SM/lib.rs:493-507`) | 15-point cost per potion |
| Boss win value | **Final boss: HP and potions carry 0**, so a win at 1 HP scores the same as a win at full HP and every potion gets drunk. Acts 1/2 boss: HP carry 0 (A0–1) or 0.2 (A2+), because the next Ancient heals (`RA/policy/mod.rs:858-871`, `1887-1906`; `sts2-core/src/lib.rs:11600-11613`) | Same weights as any fight |

Supporting tricks they measured or kept:
- **Learned playout policy** (`SM/rollout_net.rs:1-40`). A small factorized softmax: a state query
  vector dotted with an action key, where the key is a per-card (+upgrade), per-potion or
  end-turn embedding plus a projection of action features. They fitted it first to the search's
  own moves (distillation), then trained it with REINFORCE on playout outcomes starting from human
  A10 boss and elite fights. Reason given: the hand-written priority list "plays out its whole
  hand every turn; in long boss fights its playouts almost never win, the search gets no win
  signal, and the pilot plays as if the fight were already lost". Gain: **+4.67pp full-run
  Ironclad** pooled over 1,200 paired runs (A9 33.5→43.5%). Same idea for Defect: +9.8pp at A9.
- **More sims at the boss**: going from 1000 to 4000 sims at the boss is "a measured +3.7pp lever"
  (`RA/policy/mod.rs:294-297`). The knee of the win rate is the boss. Trimming sims in common fights
  to pay for more at elites lost 6pp at A10 (`:250-266`), because cheap common fights bleed HP.
- **Root budget allocation**: 47% of common-fight decisions carry a "potion available" or
  "several enemies" motif; those gained most from 4× sims (`RA/policy/mod.rs:182-188`, `340-352`).
- **Dominance pruning** (`SM/dominance.rs:1-30`): drop a play order when the swapped order is
  provably no worse. It is checked on enemy HP only, because player HP and block interact with
  relics.
- **Conservative lethal detector** (`SM/lethal_detector.rs:1-25`): a damage knapsack proves "kill
  this turn" and the search marks the node as solved. It bails whenever an unusual power is present.
- **Turn plans** (`SM/turn_plan.rs:1-13`): enumerate the deterministic action sequences up to
  EndTurn or the first chance event, deduped by the state they reach. A plan boundary is exactly a
  chance node. This is the same grouping as our beam.
- Multi-form enemies: count killing a revived form as progress (`removed_counts_forms`,
  `SM/lib.rs:580-593`). It cut Test Subject form-1 deaths from 32 to 4. We already weight
  `RESPAWN_WEIGHT`.

**What transfers.** Our biggest gap is the **horizon**. A one-turn score cannot see the boss
scaling (Strength, summons, enrage) that kills us at floors 17/33/48. Their own data says this is
where honest search loses the most (rent grows with act). The cheapest honest version we can
build: keep the K blindfolded worlds, and in each world score each candidate turn plan by
**playing the fight to the end** with a fast playout policy, averaging win/HP over the worlds
(determinized Monte Carlo, i.e. PIMC with rollouts). Our combat PPO, weak as a controller, is a
natural first playout policy; it is already honest (`crates/sts2-rl/tests/leak.rs`). After that,
the real step up is a single-tree search with chance nodes (honesty.md already names it as the
follow-up).

## 3. Run policy

Paths: `ppo/` = `sts2-pycli/python/ppo/`. Champion: `ppo_fullrun_ironclad_plr_it550`
(`sts2-pycli/data/ppo/champions.toml:42-47`).

**How much the run policy matters.** Use the same honest combat search (1000/4000 sims) and
change only the run policy. Their hand-built ranker reached 15.55% at A0. The PPO net reached
55.75% at the time (clairvoyance-rent doc, G4 table), and the release now reaches 89.5%. Without
the net, then, their strong honest combat search still wins only about 15%. That is our
ballpark at 8% with a weaker search. **Both halves matter, and run decisions are worth more than
search past about 15%.**

- **Observation**: a token set. Each token is a section embedding, plus 12 categorical embeddings
  (card, upgrade count, relic, potion, room type, event id, action kind, option index, decision
  surface, act boss, deck-op kind, enchantment), plus a linear map of 25 sin/cos-encoded
  continuous features (`ppo/schema.py:9-11,63-87,134-161`; `ppo/model.py:135-150,231-249`). The
  continuous features include HP fraction, gold, act and floor, deck size, upgrades, map dx/dy,
  min/max elites ahead, distance to rest, price, and option deltas. Vocab sizes: 599 cards, 299
  relics, 64 potions, about 71 events.
- **Enemy identity**: only the current act's boss is embedded, on the state token and on the boss
  map node (`ppo/schema.py:21-25`).
- **Map**: per-node features, plus a "forward cone" summary of the destination for each path
  option (elites ahead, distance to rest; `ppo/mapfeat.py:1-27`). Their note: leaving those
  aggregates unscaled cost about 40pp.
- **Net**: transformer with dim 192, 3 layers, 4 heads. Each candidate action is a token, and one
  shared linear head scores all candidate tokens, with illegal ones masked
  (`ppo/model.py:153-207,251-264`). Value is a mean-pooled head. An auxiliary head predicts the
  next room type at weight 0.1. The decision surfaces are path, card reward, boss/relic reward,
  remove/upgrade/transform/smith, shop (buy one item, then decide again), event, rest, and potion
  swap (`ppo/vocab.py:198-218`, `ppo/env.py:76-88`). Combat always goes to MCTS.
- **Reward**: potential-based. Φ = 0.5·floor/45 + 0.5·win, plus small shaping on non-terminal
  states (0.1·hp_frac + 0.035·upgrades − const) (`ppo/rewards.py:1-102`). Hyperparameters: γ=1,
  GAE λ=0.97, clip 0.2, policy LR 3e-5, value LR 1e-4, 2 epochs, entropy 0.05 → 0.0125
  (`ppo/trainer.py:120-142`).
- **Curriculum**: ascension sampled uniformly over A0–A10, and act 1 rolled
  (Underdocks/Overgrowth). PLR (`ppo/plr.py:1-66`) keeps (seed, ascension) "levels" scored by mean
  |GAE|, uses rank priority (β 0.1) plus staleness (ρ 0.1) with capacity 4096, and replays a level
  with probability 0.5. Neither win credits PLR: the Ironclad gain was confounded, and a clean
  Silent A/B came out null (`champions.toml:46,53`). An LR anneal tail measured +6pp, which is
  real (`docs/ppo-champion-training-playbook.md:497-506`).
- **Scale**: 960 games × 150 iterations per leg ≈ 144k runs, about 44 h on 46 CPU workers plus one
  RTX 3090. The champion is the end of a warm-started lineage that began with behaviour cloning,
  roughly 0.3–0.4M runs in total (agent estimate). Collection takes 86–97% of the wall time.
- **Combat inside training**: honest MCTS at 250 sims (1000 at a boss), with playouts from the
  STS1-style priority list (`sts2-mcts/src/expert.rs:1-75`). It was cheaper per run and **+2.5pp
  better at equal compute** than 1000/4000 with fewer games (`playbook:466-490`). Win rate rises
  steeply with the eval budget: 20% → 53% → 61% → 72% at 32/100, 100/300, 250/1000 and 1000/4000
  sims (`docs/ppo-screen-playbook.md:47-54`).

## 4. Cheap wins (honest, reimplementable as ideas)

Expected impact is on our boss deaths (floor 17 ~170, 33 ~100, 48 ~70 per 500). These are
estimates, not measurements.

| # | Idea | Their evidence | Impact on our boss deaths |
|---|---|---|---|
| C1 | **Last-boss win value = win only.** At floor 48, score a won fight the same at any HP and price potions at 0, so the search drinks the whole belt. After the act-1/2 bosses, end HP barely matters (an Ancient heals; carry 0 at A0) | `sts2-runai/src/policy/mod.rs:858-871`; `sts2-core/src/lib.rs:11600-11613` | Floors 17/33/48: today our POTION_COST of 15 and HP_WEIGHT of 100 make the search hoard potions and HP in the very fights that end the run. A few deaths each, for a one-line change |
| C2 | **Spend more search at the boss.** Their 1000→4000 boss escalation measured +3.7pp; cutting common-fight sims cost 6pp at A10 | `policy/mod.rs:294-297,250-266` | Raise our boss K/beam about 4× (we use 8/8 today) |
| C3 | **Learned per-card playout priority** (state·card-key softmax), fitted to search moves then REINFORCE from boss starts | `sts2-mcts/src/rollout_net.rs:1-40` | Only useful once we have full-fight playouts (plan step 1). +4.7pp for them, mostly at bosses |
| C4 | **Playout priority list**: end turn last; Strikes/Defends mid; powers early; on a turn where we would die, prefer block (Defend-class) | `sts2-mcts/src/expert.rs:1-75,356-420` | The default playout policy for step 1 before any learned one exists |
| C5 | **State value V for run choices**: HP·(1+act progress) with an extra penalty below 50% HP; per-card damage/(1+cost) and block; +AoE (more vs Colony/Swarm Mother); +scaling powers; −curses; −0.02 per card above 20; relic ≈0.5 (raising 0.1→0.5 gave +3.3pp in their shop tests); potion 0.05 plus boss insurance | `sts2-runai/src/eval/heuristic_v.rs:46-110`, `data/runai/heuristic_v.toml` | Floor 17: deck strength at the boss is the ceiling, not HP bleed (their event-pilot doc :201-204); fewer, better cards and more relics won more |
| C6 | **Shop by ΔV per gold**, removal included, bought greedily one item at a time; keep 75 gold | `policy/surfaces/shop.rs:1-11` | Small; mostly thinner decks for 33/48 |
| C7 | **Rest**: heal only if 30%·maxHP is worth more ΔV than the best upgrade | `policy/surfaces/rest.rs:18-35` | Small; we already do a threshold rule |
| C8 | **Path by projected V at the boss**: enumerate every path; expected HP loss per room (monster 8/12/16, elite 18/26/34 by act; event 2; ? 4); room reward ΔV (elite 0.18, treasure 0.12, monster 0.06…); big penalty for paths that reach 0 HP | `policy/surfaces/pathing.rs`, `heuristic_v.toml` | Floor 17/33: arrive at the boss with more relics and HP |
| C9 | **Probe close card picks with search**: when the top-2 margin is small, play 32 synthetic-seed fights against the next elite and the act boss with each deck variant and keep the winner. Honest: synthetic RNG, no real next fight | `sts2-runai/src/policy/tiered.rs:1-22`, `rollout.rs:1-30` | Directly targets "the deck can't beat this boss"; cost scales with our combat speed |
| C10 | **Upcoming boss in view**: the act boss is visible, so tilt picks (AoE vs Colony/Swarm Mother) | `ppo/schema.py:21-25`, `heuristic_v.rs` | Floor 17/33; cheap |

Do not copy: their event surface for the heuristic agent is "option 0", and their lethal-event
veto previews the outcome on a clone. Our text/expected-value table is already better and honest.

## 5. Ranked plan (expected gain per effort)

1. **Boss-aware win value + boss budget (C1, C2).** In boss fights: HP carry 0, potion cost 0,
   especially at floor 48; 4× worlds/beam at bosses. Hours of work. Measure on the 500 dev seeds;
   expect fewer floor-48 and 33 deaths, perhaps +2–4pp.
2. **Full-fight playouts in the blindfolded worlds (PIMC with rollouts).** For each candidate
   action, in each of K `blindfold_for_search` worlds, finish the turn with the beam, then play to
   the end of the fight with a fast playout policy (C4 list first, then our combat PPO, then a
   learned one per C3). Score wins as 1 + small HP/potion terms and losses by how much enemy HP
   remained. This removes the one-turn horizon that cannot see boss scaling. Must pass the
   action-leak gate (playouts run only in the blindfolded copies). Days of work; their
   priority-list→learned-playout step alone was +4.7pp, and the horizon fix should be worth more
   for us.
3. **A real state-value V for run decisions (C5, C8, C10)**, replacing tier-only picks: pick or
   skip by ΔV, path by projected V at the boss, relics valued at about 0.5 each. Days. Targets the
   floor-17 deck ceiling.
4. **Search-probed close picks (C9)** once combat search is fast enough: boss fights simulated on
   synthetic seeds decide close card/relic picks. Days; the cost is compute.
5. **Then a single-tree chance-node search** (their BeliefDraw design: sort the unknown pile, draw
   at random when drawing, reseed streams per chance outcome, DPW on draws and EndTurn, PUCT with
   c rising with budget, regularized root π). Their F0 (one fixed world, like ours) → F2 step was
   +3.75pp for the ranker and +6.6pp for PPO. Weeks.
6. **A run PPO over vocab embeddings** (section 3) with cheap honest combat in the loop. Ranker vs
   PPO under the same search was 15.55% → 55.75% for them, the largest single lever, but it needs
   about 10⁵ runs per leg, so only once combat is fast and strong. Weeks plus compute; keep event
   option features text-derived (not cloned previews) to stay honest.
