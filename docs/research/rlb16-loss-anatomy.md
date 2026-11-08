# RLB16: anatomy of the remaining losses

**Result:** no change beats the default; it is unchanged. Of 120 losses of the default on train seeds 1,000,000–1,001,999, **82 (68%) are
search-losable** (a clairvoyant or a 128-world honest search wins the fatal fight from the same
start) and **38 (32%) are pre-decided** (no search tried wins it). But 56 of the 82 are won by an
honest search that ties the default overall (RLB15), so most search-losable losses are close
fights the default lands on the wrong side of, not a shared mistake. The pre-decided ones enter
their boss with fewer potions, twice the curses, less max HP and more HP lost on the way. Two
fixes, each against the clearest signal, were flat in the paired A/B: saving potions for the boss
(939 vs 943, p 0.60) and removing curses at shops first (547 vs 553 of 592, stopped). Then, on
the Queen and Test Subject (47 of the 82 search-losable losses), turn-by-turn traces against the
clairvoyant search pointed at one mis-scored mechanic: the search counts the Queen's amalgam (a
Minion, which dies with her) as HP to take off. Scoring it as the fight scores it lost on a new
low-noise benchmark (Queen 82.3% against 85.0% of 840 reseeded hard fights, p 0.022): the
amalgam's HP stands in for its growing threat.

The noise is large: the same bot with other sampled worlds (`--search-salt 1`) won 928 of the
same 1,000 runs against `base.tsv`'s 943 (discordant 47/32, p 0.12). `base.tsv` is a high draw
(three runs of the default: 943, 928, and 937 on seeds 1,001,000–1,001,999), so a variant that
changes runs early starts about 7 runs behind it.

Data: `target/rl-checkpoints/RLB16/` (main checkout). Code: `honest/log.rs` (`run-log`),
`honest/search.rs` (`--search-salt`, `--value-potion-road`, `--minions`), `honest/run.rs`
(`curse_removal`, `--remove-curses`), `main.rs` (`fights --reseed`, `--trace-advice`). Scripts:
`python/rlb16_*.py`.

## Noise

| run | wins of 1,000 | deaths act 1 / 2 / 3 |
|---|---|---|
| `base.tsv` (RLB14's A/B arm, seeds 1,000,000+) | 943 | 9 / 10 / 38 |
| the same, `--search-salt 1` | 928 | 9 / 14 / 49 |
| the default, seeds 1,001,000+ | 937 | 11 / 13 / 39 |

The bot is deterministic per seed (world seeds hash the observation); `--search-salt N` mixes N
into every world seed, so the same bot samples other worlds. A paired A/B of 1,000 runs moves by
±1.5% by chance alone; with 943 as the base it needs a true gain of about 2% to reach p < 0.05.

## The split

The fatal fight of each of the 120 losses, replayed from its exact start (`fights --seeds
lost-all.txt --every 1 --only last --start-policy builtin --start-value builtin`, the default's
search; it reproduces all 120 losses):

| controller | won of 120 |
|---|---|
| the default (64 boss worlds, beam 48, keep 6, mix 300) | 0 |
| clairvoyant, one true world (`c`) | 70 |
| clairvoyant, beam 128 / HP weight 0 / keep 12 beam 96 | 69 / 69 / 71 |
| clairvoyant, any of the above | 75 |
| honest, 128 boss worlds, beam 96 (± keep 12) | 45 (56 either) |
| **any** (search-losable, class a) | **82** |
| none (pre-decided, class b) | **38** |

A clairvoyant two-turn lookahead (`c l4`) ran over 45 minutes without finishing and was stopped.

| fatal fight | a | b |
|---|---|---|
| Queen | 23 | 10 |
| Test Subject | 24 | 7 |
| Aeonglass | 6 | 4 |
| Knowledge Demon | 5 | 4 |
| Lagavulin Matriarch | 0 | 5 |
| other bosses (5 kinds) | 15 | 4 |
| elites and hallway fights | 9 | 4 |

The honest 128-world search wins 56 of the 120 lost fights, yet ties the default on all boss
starts (RLB15). The default's losses are mostly close fights that another sample of worlds wins
and loses others; what a different search fixes here it breaks elsewhere.

## What separates them

At the fatal fight against wins at the same fight (`run-log` over all 2,000 runs; act 3 and acts
1–2 boss entries):

| at the act-3 boss | wins | a | b |
|---|---|---|---|
| HP / max HP | 97 / 98 | 90 / 91 | 81 / 90 |
| potions | 2.4 | 2.0 | 1.4 |
| curses | 0.5 | 0.5 | 1.0 |
| upgrades | 15.3 | 13.8 | 13.2 |
| relics | 19.0 | 17.7 | 17.3 |
| elites fought | 5.1 | 4.7 | 4.2 |
| HP lost in fights over the run | 96 | 129 | 152 |
| gold left | 139 | 118 | 175 |
| n | 1,880 | 53 | 21 |

| at an act 1–2 boss | wins | a | b |
|---|---|---|---|
| HP % | 94 | 89 | 85 |
| potions | 1.6 | 0.6 | 0.9 |
| curses | 0.3 | 0.3 | 0.9 |
| gold left | 186 | 249 | 391 |
| n | 3,760 | 17 | 12 |

Loss rate by potions at the boss: act 3, 7.7% with none, 6.6% with one, 1.8% with three; acts 1–2,
2.1% with none, 0.2% with two or more. Act-3 boss losers drink more potions in act-3 elites (1.9
against 1.5 per run). Deck size, strikes and defends, removals, shops and cards picked do not
separate them. Drinking potions on the boss's first turn does not either: wins do it more often
(Test Subject: 64% of wins' potions on turn 1, 49% of class a's).

By hand (7 Queen and 7 Test Subject losses, `fights --trace`): Test Subject losses reach the
300-HP third form (Intangible every other turn) with 20–45 HP and lose the race; the second form
(Painful Stabs) costs 30–50 HP. Queen losses take one 40–50 HP turn after the amalgam dies and
the Queen enrages. The traces show no repeated misplay that a search setting would change.

## Fixes

Both paired against `base.tsv` on train seeds 1,000,000–1,000,999.

**Save potions for the boss** (`--value-potion-road 25000 --value-potion-road-act 2`: a potion
costs 2.5% of a fight win instead of 0.5% outside boss fights in act 3): **n 1,000, base 943,
variant 939, discordant 18/14, p 0.60.** It worked as meant (potions at the act-3 boss: wins 2.44
→ 3.04, losses 1.81 → 2.51), yet act-3 deaths rose 38 → 42. Few potions at the boss mark a hard
run; they do not cause the loss.

**Remove curses first** (`--remove-curses`: at a shop with a removable curse in the deck and the
removal affordable, buy the removal and take the curse, before the run policy's choice). The run
policy removed a curse at only 59% of such visits in wins (709/1,202) and about as often in
losses (39/57), so this is a general change, not one aimed at the losers: **n 592, base 553,
variant 547, discordant 8/2, p 0.11** (stopped there, trending worse, to free the CPU).

## Queen and Test Subject

`fights --trace DIR --trace-won --trace-advice` (64 lost Queen and Test Subject fights of the
120, variants `m300;c`): each turn's state with the enemies' intents, and at each of the bot's
decisions the clairvoyant search's move when it differs; the clairvoyant's own fights beside.
The clairvoyant wins 41 of the 64. Per turn (`python/rlb16_tracestats.py`):

| fight, phase | bot: incoming / blocked / HP lost | clairvoyant (won fights) |
|---|---|---|
| Queen, amalgam alive | 20.5 / 41% / 10.0 | 20.9 / 55% / 8.4 |
| Queen alone (enraged) | 30.8 / 29% / 14.1 | 14.7 / 50% / 13.4 |
| Test Subject form 1 | 15.6 / 63% / 3.9 | 17.2 / 52% / 3.8 |
| Test Subject form 2 | 26.4 / 38% / 12.4 | 24.9 / 42% / 5.7 |
| Test Subject form 3 | 28.5 / 41% / 10.8 | 18.4 / 48% / 18.5 |

- **Queen.** The bot kills the amalgam (199 HP, `Minion`), the Queen enrages, and her 15–16 ×5
  attack kills it from 30 HP or less (1000170: −46 HP on turn 3 attacking the amalgam with Flame
  Barrier in hand; 1000257: Queen at 310 when the amalgam died). The clairvoyant puts more
  damage on the Queen while the amalgam lives (1000257: Queen 160 at the amalgam's death), and
  a Queen death ends the fight with the amalgam alive (1001779). The search scores the amalgam's
  HP as HP to take off and its death as a kill, though a Minion dies with the last primary enemy
  (`Creature::is_secondary_enemy`).
- **Test Subject.** The losses spend 3–4 turns in form 2 (attacks 10×3, ×4, ×5, ×6) and reach
  form 3 (300 HP, Intangible every other turn) with 20–45 HP; the clairvoyant leaves form 2 a turn
  sooner, with its draws known. Drinking potions on turn 1 is common to wins and losses alike.
  No shared misplay.

### Benchmark

`fights --starts bench/hard.jsonl --reseed 9`: the 170 recorded Queen and Test Subject starts of
seeds 1,022,000–1,022,999 (RLB12's `starts.jsonl`) that the default lost or ended under 40 HP in
RLB14/15's screens, each played 9 times, the first as recorded and 8 with the hidden state (draw
order, every RNG) drawn again (`Combat::blindfold_for_search`). Only the 8 fresh copies count
(the first chose the set): 1,360 fights, 26 minutes on 16 cores, paired per copy
(`bench/cmp.py`). A change that leaves a fight alone plays it identically, so the paired
difference is the change's own.

| scoring (`--minions`) | Queen (840) | Test Subject (520) | all (1,360) |
|---|---|---|---|
| `kills` (the default) | 85.0% ±2.4 | 75.8% ±3.7 | 81.5% ±2.1 |
| `primary`: a Minion's HP and kill not counted while a primary enemy lives | 82.3%, disc. 58/35, p 0.022 | same fights | 79.8% |
| `hp`: its HP counted, not its kill | 85.0%, disc. 0/0 | – | – |

Not counting the amalgam lets it hit harder (it gains Strength every turn the Queen burns for it,
while she hides behind 20 block); its HP is the turn score's only measure of that threat. The
kill term changes nothing (the value net ranks the leaves). No whole-run A/B: nothing beat the
benchmark.

## Reproduce

```
D=target/rl-checkpoints/RLB16; B=target/release/sts2-diff
A="--value builtin --value-leaves --value-hp 300 --rollout v2 --value-mix 300 --run-policy builtin"
$B bot --first 1001000 --count 1000 $A --out $D/a1                         # second 1,000
$B bot --seeds base-lost.txt --first 0 --count 0 $A --out $D/base-lost     # base's losses' actions
$B bot --first 1000000 --count 1000 $A --search-salt 1 --out $D/salt1      # noise
$B run-log --dirs $D/../RLB14/a0-m300/wins,$D/base-lost/losses --out log0.jsonl
$B fights --seeds lost-all.txt --every 1 --only last --start-policy builtin --start-value builtin \
    ${A% --run-policy builtin} --variants "m300;c;c b128;c h0;c k12 b96;w128 b96;w128 b96 k12" --out gap.tsv
python3 python/rlb16_anatomy.py log0.jsonl log1.jsonl                     # feats.tsv
python3 python/rlb16_classes.py gap.tsv.*                                 # classes.tsv
$B bot --first 1000000 --count 1000 $A --value-potion-road 25000 --value-potion-road-act 2 --out road25a3
$B bot --first 1000000 --count 1000 $A --remove-curses --out curses
python3 $D/paired.py $D/base.tsv curses/progress.tsv
V="${A% --run-policy builtin}"
$B fights --seeds qts-lost.txt --every 1 --only last --start-policy builtin --start-value builtin $V \
    --variants "m300;c" --trace traces2 --trace-won --trace-advice --out qts.tsv
python3 python/rlb16_tracestats.py bot traces2/*.m300.txt
$B fights --starts bench/hard.jsonl --first 1022000 --count 1000 --only act3 $V --reseed 9 \
    --minions kills --out bench/base.tsv                                    # benchmark (also primary, hp)
python3 python/rlb16_bench_cmp.py bench/base.tsv bench/primary.tsv
```

## Next

1. Judge fight changes on the reseeded benchmark first (`--reseed`, ±2% in 26 minutes, exactly
   paired), and widen it: all 2,877 recorded act-3 boss starts at K = 2 is about 2 hours.
2. Compare against more than one base. A 1,000-run paired A/B has ±1.5% of noise and `base.tsv`
   is a high draw; pool the base with `salt1` (or run each arm on 2,000 seeds) before calling a
   change flat or a win.
3. The pre-decided third differs from wins in deck strength as a whole (upgrades, relics, max HP,
   HP lost over the run), not in one decision. Moving it needs a better run policy, not an
   override: continue `python/train_run_ppo.py` with the current fight bot (the policy was
   trained over RLB8's and RLB9's searches, before the value net), or a larger run net.
4. The search-losable two thirds are mostly coin flips that a different world sample wins; only
   about 26 of 120 need information the bot lacks (clairvoyant only). A stronger fight player
   (RLB15's 76% playout ceiling) is the remaining lever there. The Queen and Test Subject traces
   show HP lost to unblocked attacks, not a rule the search misreads.
