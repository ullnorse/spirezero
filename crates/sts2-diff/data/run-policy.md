# run-policy.bin

The learned run policy's weights (`honest/policy.rs`, `docs/specs/run-policy.md`, 1.7 MB).
`RunNet::builtin` loads them: `sts2-diff bot --run-policy builtin` and the action-leak gate.

Provenance: RLB8's policy (below), then RLB9's PPO on top of it.

RLB9 (train seeds only, fresh Adam, learning rate 4e-5 annealing, 1,024 runs per iteration,
`python/train_run_ppo.py --init-bin` / `--resume`): 26 iterations on the RLB8 search, then 34
on the search with the Waterfall Giant fix (`honest/search.rs`, `standing_hp`); 89,088 runs.
Chosen: iteration 60, cheap-search dev 8,000,000–8,000,499 greedy 372/500 (start 351 on the
fixed search; 359–372 from iteration 10 on, a plateau). Full search: dev 813/1000.

RLB8 (train seeds 1,000,000–1,099,999 only, A0 Ironclad, fights by the search at
4/4/16 worlds and beam 12):

1. Behaviour cloning on 8,192 runs of the hand rules (`honest/run.rs`): 630,673 decisions,
   8 epochs, held-out top-1 agreement 93%.
2. PPO, 165 iterations of 1,024 sampled runs (`python/train_run_ppo.py`, resumed from
   `last.pt` after each engine fix; learning rate 5e-5, then 1e-4, 9e-5 and 6e-5 annealed to
   1e-5). 177,152 training runs in all.
3. Model choice on dev 8,000,000–8,000,499, greedy, the same cheap search: this is iteration
   146, 323/500 (BC 85, hand rules 86).

The export is `STS2RUN1`, the feature layout's version and sizes, the vocabulary hash
(informational: the token blocks have fixed sizes and vocabulary ids are append-only), then
the f32 tensors in `RunNet::from_floats` order. Retrain after a change to `policy::Features`.
