# RLB9: the lost dev runs, triaged

The 187 runs `it60` (RLB9's run policy, the full honest search, the Giant fix) lost on dev
8,000,000–8,000,999 (813 won). Per loss: the fatal fight replayed from its exact start (the
driver reproduces the run: 0 of 187 won again), with 256 boss worlds and with a **clairvoyant**
search (sees the real draw order and RNG; a diagnostic, never the bot); and the whole seed re-run
8 times with the run policy sampled (honest) and once with clairvoyant fights. Data:
`target/rlb9/losses.tsv` (seed, floor, fight, HP in, max HP, deck, w256 won, clairvoyant won,
sampled wins of 8, clairvoyant run won, class).

| class | runs | meaning |
|---|---|---|
| (a) search blunder | 81 | the same fight start is won by 256 worlds or the clairvoyant search |
| (b) scoring bug | 0 new | the Waterfall Giant (fixed first: 8 → 26 of 26 starts) was the one found |
| (c) weak deck / HP going in | 103 | not won from that start, but a re-run of the seed wins |
| (d) engine bug | 0 in dev | two `may_prompt` panics on train seeds (Cascade; the auto-play models), fixed |
| (e) unwinnable (estimate) | 3 | no variant, re-run or clairvoyant run won |

By fight (a/c/e): Test Subject 24/31/2 of 57, Queen 16/16 of 32, Aeonglass 9/8 of 17, Knowledge
Demon 10/2, The Insatiable 7/4, Kaiser Crab 1/10, Soul Fysh 2/4; 35 elite and common deaths.

- **The seeds are not hard, the play is noisy.** Each sampled honest re-run wins 92–104 of the
  187 lost seeds; 183 are won by at least one of 8. Only 3 seeds (0.3%) lose under everything:
  the honest-winnable ceiling estimate is **≥ 99.7%**, an upper bound, not a policy's rate.
- **Clairvoyance is not the gap.** Clairvoyant fights with the same run policy win 95 of the 187
  (a ceiling diagnostic); a perfect-information search still loses half, so the search algorithm
  (one-turn beam, one priority playout per first action) limits more than hidden information.
- `reach` (clairvoyant, hand run rules) wins 1 of the 187: its run choices are far weaker than
  the learned policy's, so it says nothing about the ceiling here.
- Most boss losses go in healthy (Test Subject: 89% HP, 31 cards) and end close (boss at 12–18
  HP of 379–512). Potions were used: 178 of 187 died with an empty belt.

## Fixes tried by class

- (b) The Giant: kept (8 → 26 of 26 starts). No second scoring bug in the 187 traces: Queen
  losses come after the amalgam dies (27 of 32), Test Subject losses in forms 2–3 with decks of
  31–40 cards clogged by Wounds and Burns, most fights lost by a few turns of damage.
- (a) Search: 256 worlds, beam 48, the learned playout, weight sweeps, minion weighting,
  leader-first playout targets, block-first playouts and one-turn-plan playouts were flat or
  worse honestly (block-first won boss starts but lost runs: 1,599 against 1,650 of 2,000).
- (d) `may_prompt`: Cascade, then every auto-play model; regression tests in `sts2-core`.
- (c) Run policy: 89k more PPO runs were flat (cheap-search dev 359–372/500).
