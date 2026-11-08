# Referee performance (T13)

Warm `referee.py serve`, machine load ~5-10, timed in python around each command (game + JSON + pipe). Forced corpus: 40 Necrobinder seeds of `FUZZY_WURM_CRAWLER_WEAK` (~25 actions each) run twice, first 3 seeds dropped.
Run corpus: 12 run-mode seeds (random, greedy, shopper with the real-game branches, one run loadout). Profiles from
`dotnet-trace` (`dotnet-sampled-thread-time`) plus a scratch build that times every `Thread.Sleep`.

## Breakdown before (build `5aa161ead00e`)

Forced fight, 91-125 ms per seed (median 61-74; one seed in 40 costs ~690 ms):

| Part | Per seed | Notes |
|---|---|---|
| `start_run` | 60-98 ms | almost all `EnterAct` -> `StandardActMap` -> `MapPathPruning.PruneAndRepair` (`FindMatchingSegments`, path enumeration, seed-dependent up to 670 ms). `GenerateRooms` (all acts) < 1 ms, Neow < 1 ms |
| actions | ~17 ms | `play_card` 0.7 ms, `end_turn` 1.2 ms; game logic plus sts2-cli's own response (`CombatPlayState`, `PlayerSummary`, intent forecast) |
| `dump_state` | ~8 ms | 0.3 ms per action; dictionary build + reflection-based `System.Text.Json` |
| waits, logging, IPC | ~0 | combat path has no sleeps; stderr goes to /dev/null; pipe round trip is inside the numbers above |

Run mode, 1640 ms per seed: ~80% is `Thread.Sleep`. Our `WaitForEventTask` waited for "no card reward pending", which stays
false when the game offers the next reward at once, so it ran to its 2 s timeout (4 times in 12 seeds, 8.7 s); the rest
site's fixed 200 ms sleeps (33 calls, 6.6 s); the 10 ms polling ticks of the event/rest/reward waits.

## Fixes, ranked (gain on warm per-seed cost; parity risk)

1. **Skip map pruning in forced fights** (done). `start_run` 60-98 ms -> 3 ms. Risk low: the map draws only from its own
   `act_1_map` Rng (not a `RunRngSet` stream, so no traced counter), `GenerateRooms`/UpFront and Neow are untouched, and a
   forced fight never reads the map. Patcher: `PruneAndRepair` returns when `STS2_FORCED_FIGHT` is set; `start_run` sets it
   from `forced_fight`, which `referee.py` sends only with `encounter`. Run mode and natural fights keep the real map.
2. **`TieredPGO=false`** (done, `Sts2Headless.csproj`). Without the pruning work, dynamic PGO left the combat code about 2x
   slower (`CombatState.Creatures` LINQ). Actions + dump ~40 ms -> ~24 ms. Risk none: code generation only.
   `TieredCompilation=0` is similar warm but +1 s cold; `QuickJitForLoops=0` is weaker.
3. **Run-mode waits** (done). `WaitForEventTask` waits until the answered reward is taken (reference change); the rest site's
   two fixed 200 ms sleeps wait for the option task itself (`tracer/RunWaits.cs`); the polling loops tick every 1 ms with 10x
   iterations (same timeouts, same conditions). 1640 -> 380 ms per seed. Risk low: each wait still ends on its condition,
   and waiting for the rest task to finish is stricter than hoping 200 ms is enough.
4. Faster dump writer (`Utf8JsonWriter` by hand, no dictionaries): at most ~4 ms of ~30 (~13%). Not done.
5. Trim sts2-cli's per-action response (intent forecast walks the monster state machine; python only reads `decision`):
   maybe ~5 ms. Not done: the forecast runs game code, so dropping it needs its own parity check.
6. ReadyToRun (crossgen of `sts2.dll`): cold start only, warm code is re-jitted anyway. Not done.

## After (build `689303afa626`)

| | Before | After |
|---|---|---|
| Forced fight, warm, per seed | 91-125 ms (median 61-74) | 29-40 ms (median 29-39) |
| Run mode, warm, per seed | 1640-1810 ms | 380 ms |
| Cold start (first seed) | 1.4-2.2 s | 0.9-1.4 s |
| `tools/diff.sh 15 SlimesWeak` total | 4.1 s | 2.5 s |

Forced fight after: actions ~16 ms, `dump_state` ~7 ms, `start_run` 3 ms, `enter_room` + `reset` 1 ms.

Proof: 83 jobs (40 forced Necrobinder; Ironclad, Silent, Defect, Regent and Necrobinder loadouts on two encounters at
ascension 0/5/10; random, greedy and shopper (real-game branches) runs and a run loadout; 33 rest options, 12 Smith picks,
8 event card rewards) are byte-identical between the old build fresh per seed (`run.sh`) and the new build both fresh and
warm (`serve`), also with 4 shuffled warm processes in parallel; the 10 committed `referee/examples/*.trace.jsonl` replay
byte-identical.

Side finding: the referee runs on .NET 10 (`RollForward=LatestMajor`), where Harmony is unsupported, so every sts2-cli
Harmony patch (Task.Yield, Cmd.Wait, TalkCmd, RewardsSet.Offer, ...) fails at start-up with a `[WARN]`. Traces are the game
without them; the IL patches on `sts2.dll` still apply.
