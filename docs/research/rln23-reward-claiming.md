# RLN23: reward claiming (RLN20 suspect 5)

Measured with `python/rln23_rewards.py` on 500 DEV runs (seeds 24,000,000-24,000,499), greedy,
`evalpins/state-now.pt` (sha256 prefix in the JSON), weights `ema0.999`; 6,206 reward screens
(12.4 per run). Env unchanged.

| reward | shown / run | claimed | skipped | left at Proceed |
|---|---|---|---|---|
| gold | 11.52 (204.4 gold) | 11.52 | 0 | 0.002 (10 gold in all) |
| potion | 6.19 | 4.45 (71.8%) | 1.75 | 0 |
| card | 12.03 | 11.64 picks + 0.004 alt | 0.40 | 0 |
| relic | 1.25 | 1.23 | 0 | 0.012 (6 in 500 runs) |
| given card | 0.24 | 0.21 | 0 | 0.03 (15 in 500 runs) |

- Gold: claimed 99.99% (10 of 102,182 gold left, one screen). Nothing to gain from auto-claiming.
- Potions: of the 873 skipped, 849 had a full belt (not claimable: `Run::push_reward_actions` lists
  `ClaimReward` only with a free slot, and `DiscardPotion` has no action index). The other 24 are in
  6 runs (max 6 per run), the signature of a relic that vetoes procuring potions
  (`should_procure_potion`), not of a policy choice. With a free claimable slot the policy
  claimed every potion. Auto-claiming potions "when a slot is free" would change nothing.
- Leaving early: 24 `Proceed`s (0.05 per run) with a reward still listed, 22 with something
  unclaimed: 6 relics and 15 given cards (and the one gold) in 21 runs: noise-level.
- Real difference from spire-mentor: the 1.70 potions per run lost to a full belt. The policy
  holds potions rather than spending them (24 uses on a rewards screen in all). Whether
  mentor's belt turns over faster is a potion-use question, not a claiming one.

Verdict: no. Claiming is not a leak (gold 0.02 / run, potions 0 / run avoidable); do not add auto-claim.
