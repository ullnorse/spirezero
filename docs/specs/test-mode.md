# TestMode.IsOn: what it changes (R5)

The referee (`sts2-cli`) sets `TestMode.IsOn = true` once, in `EnsureModelDbInitialized`
(`referee/vendor/.../RunSimulator.cs:4163`, called by `StartRun`). Our engine targets the real game
(`TestMode.IsOff`). Audit of all 322 uses of `TestMode.IsOn` (239 in `Nodes.*`), the 64 of `IsOff`, and every `AssertOn`
in `ref/decomp` (233 files). (R5)

## Classification

| Class | Hits | Where |
|---|---|---|
| Cosmetic: VFX, audio, tweens, FTUE, screens, popups, timers, nodes | ~330 | `Nodes.*` (all 239 IsOn and ~40 IsOff), `Commands/VfxCmd`, `CardCmd` previews, `CardPileCmd` tweens, `Creature.Get*Node/Vfx*`, `CreatureCmd` anims and game-over screen, monster/card/power/potion VFX branches (`Vantom`, `Myte`, `SandpitPower`, `HandOfGreed`, `RollingBoulderPower`...), `PreloadManager`, `EventModel.GetAssetPaths`, `MapRoom.Enter`, `MapCmd` |
| Saves, telemetry, network, logging, mods | ~25 | `SaveManager`, `ProgressSaveManager`, `ChecksumTracker`, `CombatReplayWriter`, `Logger`, `LocManager`, `ModManager`, `NetCursorHelper`, `PeerInputSynchronizer`, `NetCombatCardDb`, `RunManager:473,488,1377,1691`, `MerchantRoom.Exit:74` (map-point history only) |
| Timing only (`Task.Delay` vs scene timer, pause loop) | 3 | `CombatState:605`, `ActionExecutor:235`, `RunManager.FadeIn/FadeOut` (TestMode runs the `TestFadeIn` hook) |
| Test-only guards (`IsOff` throws, `AssertOn`) | 7 | `CardModel:1260`, `AbstractModel:2469`, `Slither:22`, `SneckoOil`, `SneckoEye`, `FakeSneckoEye`, `ConfusedPower` (energy-cost test overrides) |
| **Gameplay** | **7 (+3 inert)** | table below |

## Gameplay hits

| # | file:line | In test mode | RNG stream | Engine |
|---|---|---|---|---|
| 1 | `Entities.Merchant/MerchantPotionEntry.cs:67` (`IsOff`) | Shop potion keeps its base price (50/75/100); off: `round(cost * Shops.NextFloat(0.95, 1.05))` | `PlayerRng.Shops`, 3 draws per shop | **Modelled**: `Run::in_test_mode`, `run/shop.rs:312` (RL6). **Diffed in the real-game branch** (H3): `"real_shop_prices"` below |
| 2 | `Relics/Cauldron.cs:40` | Reward is 5 fixed potions (Flex, Weak, Vulnerable, Strength, Dexterity); off: 5 `PotionReward(Owner)` rolled | `PlayerRng.Rewards` (`PotionReward.cs:56`), draws per potion | **Modelled** (RB): `content/relics/cauldron.rs` behind `RunState::test_mode`. Referee switch `"real_cauldron"` (H3); no diff run has reached it yet, see below |
| 3 | `Relics/CallingBell.cs:39` | Reward is the fixed relics Anchor, GremlinHorn, MummifiedHand; off: Common, Uncommon, Rare `RelicReward`s | relic rolls (`RelicReward`) from the relic grab bags | No (unported). Referee switch `"real_calling_bell"` exists (H3), nothing to diff until the relic is ported |
| 4 | `Runs/RunManager.cs:773-779`, caller `:757` | `ShouldApplyTutorialModifications()` is false: no `ApplyDiscoveryOrderModifications`. Off: for an unseen encounter the act boss (and act-specific encounters) is replaced by the discovery order | none (replaces rooms after `GenerateRooms`) | No. Depends on the profile's `UnlockState.HasSeenEncounter`; the engine assumes everything seen, so equal to test mode |
| 5 | `Runs/RunManager.cs:1003` (`TryGetRoomTypeForTutorial`) | Off: on a profile with `Progress.NumberOfRuns == 0`, the `Unassigned` start point becomes an Event room | none | No. First-ever run only; engine assumes `NumberOfRuns > 0` |
| 6 | `Commands/CardSelectCmd.cs:978` (`FromChooseABundleScreen`) | Picks `bundles[0]`; off: the player picks. `ScrollBoxes` is the user. sts2-cli's Harmony patch of this method fails on this runtime (see RL4 note), so the referee always takes bundle 0 | none | **Modelled** (RL7): `RunCtx::choose_a_bundle` takes `bundles[0]` in test mode, else shows `RunDecision::Bundles` |
| 7 | `Rewards/RewardsSet.cs:172` | `Offer` selects rewards through `testSelector` or auto-claims every reward; off: waits for the UI. The referee's selector takes what fits and skips the rest (`referee/tracer/Rewards.cs`, RF2) | none | N/A (flow) |
| - | `Relics/LargeCapsule.cs:40,49`, `ScrollBoxes.cs:108` | Swap in Ironclad cards for the `Deprived` test character | - | Inert: never a real character |
| - | `Entities.RestSite/RestSiteOption.cs:55` | `generateForTests` hook, null unless a test sets it | - | Inert |
| - | `Models/ActModel.cs:550` (`GetRandomList`, `IsOff`) | Off: the first undiscovered alternate act is forced | `UpFront`: no draw when forced | Inert for the referee: sts2-cli builds the run with `RunState.CreateForTest`, not `StartRunLobby` |

`MoveToMapCoordAction:37` (in test mode it calls `EnterMapCoord` directly instead of animating the map screen) enters the same room
and is not a state difference.

## Can the referee run with TestMode off?

**No.** Tried: copy of the build with `TestMode.IsOn = false` (`STS2_TESTMODE_OFF=1`, `RunSimulator.cs:4179`), `T6S0 run_act1.json`:
`start_run` dies in `RunManager.FadeOut` (`NRun`/`NTransition` Godot nodes are null: `NullReferenceException`) before the first map.
That is the first of the ~240 `Nodes.*` / `NCombatRoom.Instance` / `NRun.Instance` dependencies that TestMode skips (room `Enter`, combat start,
rewards, merchant, events); sts2-cli's `GodotStubs` have no scene tree. Stubbing them is a rewrite of the harness, not a flag, and
Harmony does not work on this runtime, so a method cannot be redirected at runtime either. The on-run (`T6S0`, test mode on) completes: 144 trace lines.

## Opt-in real-game branches in the referee (H3)

`referee/patcher` (Mono.Cecil, run by `referee/setup.sh` after the sts2-cli patches) rewrites exactly the three call sites above so that an
environment variable switches each to its real-game branch; with the variables unset the build behaves as before. Actions-file keys (run mode
only; `referee.py` sets the variable): `"real_shop_prices": true` (`STS2_REAL_SHOP`, `MerchantPotionEntry.CalcCost` takes the `IsOff` branch),
`"real_cauldron"` (`STS2_REAL_CAULDRON`), `"real_calling_bell"` (`STS2_REAL_BELL`). Nothing else changes: the run is still in `TestMode.IsOn`
(Nodes, saves, flow) and still takes bundle 0, so these are the only gameplay sites made real. The engine side is `Run` without `in_test_mode`;
`REAL_SHOP=1 POLICY=shopper tools/diff_run.sh 50` plays both sides that way (`run-gen --real-game 1` writes `real_shop_prices`,
`real_cauldron` and, since PX15, `real_calling_bell`; run fixtures read `real_shop_prices` to pick the engine mode). A bundle choice (ScrollBoxes) stops a real-game run: the
referee can't answer it. 50 seeds match, including 21 shops with 9 potion purchases; the flag is live (the same actions on the unpatched
branch diverge on `Shops` at the first shop). `Cauldron` needs a shop that stocks it and about 200 gold: 640 random-policy seeds never bought
one, so its real branch is unit-tested only (`run/tests_rb.rs`).

## Recommendation

Keep TestMode on in the referee and model each gameplay branch behind `Run::in_test_mode`; it is already the pattern for #1.

1. Referee-diff sessions use `in_test_mode()` (already so). Real-game sessions (training, the actual product) use the default.
2. Port Cauldron and CallingBell with **both** branches behind `s.test_mode` when their tasks run (#2, #3). Each has a fixed list in test mode
   that needs only a table, no RNG; the off branch is the rolled one and needs the same stream draws as the game.
3. #4 and #5 stay unmodelled: they depend on profile unlocks, and the engine models an experienced profile, which matches test mode.
4. #6: the referee cannot test bundle choices other than 0; restrict diff scenarios with ScrollBoxes to bundle 0 (or accept the gap) until the
   adapter can answer the bundle prompt. Fixing this needs an IL patch like sts2-cli's `Mono.Cecil` ones in `setup.sh` rather than Harmony.
5. The potion-price roll (#1) is checked against the real game by the H3 patch above (done).
