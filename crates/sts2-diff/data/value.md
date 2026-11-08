# value.bin

The combat value/policy net's weights (`honest/value.rs`, `python/sts2/combat_net.py`, 4.8 MB):
embeddings of cards, monsters, powers, intents, relics, potions and the encounter over the
leak-tested combat observation, a 1,552 → 512 → 512 ReLU MLP, and heads for the policy (140
logits), P(win the fight) and the HP fraction kept. `ValueNet::builtin` loads them:
`sts2-diff bot --value builtin --value-leaves --value-hp 300`, and the action-leak gate.
`value-check.bin` is the parity fixture (64 positions with PyTorch's outputs).

Provenance (RLB11, train seeds only, A0 Ironclad):

1. Fight starts: `sts2-diff collect-starts --first 1010000 --count 3000 --run-policy builtin`
   (the RLB9 bot), 62,655 starts. `python/rlb11_starts.py` holds out the last 150 runs and
   shards the rest by 200 runs, with elite starts repeated 3 times and boss starts 6 times.
2. Masked PPO on the GPU (`python/train_combat_puffer.py`, PufferLib 3.0.0's `PufferEnv` and
   advantage kernel; 4,096 envs, horizon 65, each episode's hidden part resampled): 300M steps
   on the first 1,225 runs' shards, then 350M more on all 2,850 (fresh Adam, learning rate from
   1.6e-4 annealed). Chosen: the last checkpoint, update 2,479 (650M steps).

Its own greedy play on held-out starts: commons 99.3%, elites 97.9%, bosses 88.9%; P(win) at
the start predicts 99.6%, 98.3% and 88.8%. Regenerate after a change to the observation or
the vocabulary (`ValueNet::builtin` refuses an export for another one).
