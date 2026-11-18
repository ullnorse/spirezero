# RLN35 part B: forced rows in the e2e update

Tool: `python/rln35_forced_rows.py` (reads a scratch copy of NECRO3's `pinned/state-100M.pt`, resumes its
env position with its weights, no update; NECRO3's frozen lib; GPU under the lock, 2 cores, 3 GB cap).
12 collects × 2048 envs × 16 steps = 393,216 rows at 100M decisions (practice on, pool empty at start).
Rows with an empty mask (no decision) are excluded.

| | rows | mask sum == 1 | p(max action) ≥ 0.99 | tokens/row |
|---|---|---|---|---|
| combat | 312,109 | **0** | 22.4% rows, 23.6% tokens | 36.2 |
| run | 81,107 | **0** | 66.7% rows, 63.6% tokens | 39.8 |
| all | 393,216 | **0** | 31.5% rows, 32.5% tokens | 36.9 |

## Forced rows: none, by construction
The env already auto-advances single-class decisions (WP1, `docs/plans/necro-e2e.md` G2/auto-advance): a step with one
legal class gets no forward pass and no row. So no trained row has exactly one legal action; the share of update
compute is 0% and **dropping them would gain 0% dps**. The ≥ 15% threshold is not met; `skip_forced` is not implemented.
The measurement (count of mask bits per collect row) would show any regression if the env's auto-advance changed.

## Near-deterministic rows
Rows whose policy already puts ≥ 0.99 on one action are 31.5% of rows (run rows 67%). They are not free to drop:
they carry advantage-weighted gradient (zero only if p = 1 exactly), clipping and the entropy term still act on
them, and dropping them biases the per-type advantage normalisation and the entropy mean (the collapse rule's
input). Update time is ~82% of an iteration (collect 18%, NECRO3 stats over its last 400 iterations), so an
upper bound for skipping all of them is ≈ 0.325 × 0.82 ≈ 27% of iteration time, not a safe or measured gain;
not pursued.

## Entropy rule vs NECRO3's current entropy
Last 400 iterations (to 134M decisions): `ent_combat` 0.31–0.34 (min 0.305), `ent_run` 0.10–0.13 (min 0.100).
Defaults (0.10 combat, 0.05 run) are never close. The coefficient decay starts at 374M, so collapse risk grows later;
`ent_boost` is inert until a type sits under its floor for 20 iterations.
