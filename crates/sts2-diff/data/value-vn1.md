# value-vn1.bin

VN1's combat value net (`--value vn1`; the default stays `value.bin`). Same format and
architecture as `value.bin` (1,552 → 512 → 512, policy/win/HP heads), 4.8 MB.

Provenance (train seeds only, A0 Ironclad): `value.bin` (RLB11 `run2/last.pt`) fine-tuned by
`python/vn1_train.py --data RLB12/vc-a --init RLB11/run2/last.pt --epochs 8 --lr 5e-5`
(AdamW, one-cycle, minibatch 4,096, elite rows ×3, boss rows ×6, KL 1.0 to the start policy),
epoch 8. Targets: each turn-start position's fight outcome (win; HP fraction kept on a win)
under the RLB12 bot, 290,355 positions from seeds 1,020,000–1,022,999; seeds
1,022,000–1,022,449 held out (the screen's starts come from them). sha256 prefix
`b5e31790e7e62771`. Held-out boss rows: BCE 0.116 → 0.083, mean P(win) 0.925 → 0.965 vs 0.965
won; `docs/research/vn1-value-net.md` has the screens.
