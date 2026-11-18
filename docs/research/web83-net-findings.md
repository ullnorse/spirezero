# WEB83: the CB3 combat net in the browser (Strike over Unleash, missed lethal)

The browser path matches the training env end to end (`crates/sts2-web-bot/tests/e2e_parity.rs`):
400 panel fights, 17,187 decisions, and 120 whole runs (A10 and A0) through `Game` + `CombatNet`,
17,184 fight decisions: 0 differences in obs bytes, mask words, chosen index or game action.
So both owner reports are about the net (CB3 lane L, 759M, EMA 0.996), not the web code.

## Strike over a playable Unleash
Unleash deals 6 + Osty's HP (`content/cards/unleash.rs`), and the obs preview shows that sum.
- Panel, 400 fights: 1,283 decisions with both playable; Strike played 38 (17 with Osty ≥ 3 HP
  and Strength ≤ 2); the policy rates a Strike above every Unleash in 209 (16%).
- Whole runs, A10, 60 runs: 1,501 decisions; Strike 83 (36 with Osty ≥ 3 HP and Strength ≤ 2);
  Strike rated above Unleash in 335 (22%). There are clear errors, e.g. Strike at p = 0.996 over
  Unleash with Osty at 6 HP, and Strike at p = 0.72 with Osty at 19 HP.
- The owner's case (RUN5, A0, floor 3, Shrinker Beetle, turn 1, Osty 1/1, so Unleash hits for 7):
  Strike 0.562, Unleash 0.425, Defend 0.013, the same in Rust native and in the browser.

## Lethal not taken
`tests/lethal`: exhaustive search over the turn's card plays and choices (no potions), at most 4,000
positions; "honest" = the line also wins on two determinized copies. Margin = previewed damage
minus enemy HP + block (approximate).
- Panel, 400 fights: 169 honest lethal turns, 20 missed (12%). Misses by margin [0-2, 3-9, 10+]:
  [6, 4, 10] vs taken [49, 30, 70]. Delay from the first lethal turn to the kill: mean 0.21 turns,
  max 6. The search hit its cap on 894 of 3,069 turns (big hands), so lethal turns are undercounted.
- Whole runs, A10, 60 runs, 344 fights: 388 honest lethal turns, 139 missed (36%). Misses by
  margin [66, 50, 23] vs taken [44, 116, 89]: the miss rate is 60% with margin 0-2, 30% with 3-9
  and 21% with 10+. By cards [1, 2, 3, 4+]: [11, 11, 106, 11] vs taken [34, 51, 147, 17].
  Delay: mean 0.65 turns, p90 2, max 13.
- Deaths after an honest missed lethal: 0 of 65 deaths on the panel and 0 of 59 in whole runs. A
  within-turn lethal check would have saved no fight in these samples. The cost is turns and HP.
- Value head (`python/web83_value.py`): at missed turns V = 0.973 on the panel (0.997 in whole
  runs), and a turn later 0.987 (1.000). Killing now returns win_bonus 1.0, so the value head is
  indifferent to stalling.

## Suspected cause (training, not changed here)
Gym returns: γ = 1, terminal `win_bonus` 1.0, `hp_coef` 0.3 × HP fraction change, nothing per
turn; a 100-turn cap pays 0 (`train_e2e.py` `_gym_reward`). While Osty soaks the hits, stalling
costs nothing, so neither the policy nor the value learns time pressure. In whole runs, near-exact
lethals (margin 0-2) are missed 60% of the time, so the net also counts exact lethal poorly.
