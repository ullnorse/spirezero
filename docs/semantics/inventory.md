# Semantics: content inventory

Append-only. One fact per bullet, with a decomp reference (`ref/<path>:<line>`) and the task id that found it. Index: `docs/semantics.md`.

## Content inventory (T4)
Generated data in `docs/inventory/act1.{json,md}` by `tools/inventory.py`. Facts the scan relies on:
- The Necrobinder potion pool has no list of its own: `NecrobinderPotionPool.GenerateAllPotions` returns `Necrobinder4Epoch.Potions` (PotionOfDoom, PotOfGhouls, BoneBrew). Epoch classes (`Timeline.Epochs/*Epoch.cs`) are also where the relic/card unlock gates live. (T4) `MegaCrit.Sts2.Core.Models.PotionPools/NecrobinderPotionPool.cs`
- A hook is an overridden method; the listener hooks are the 183 `public virtual` methods of `AbstractModel`, and kinds add lifecycle ones (`AfterObtained`, `AfterApplied`, `AfterAddedToRoom`...). `OnPlay`/`OnUse`/`OnUpgrade` are the per-kind entry points, not listener hooks. (T4) `MegaCrit.Sts2.Core.Models/AbstractModel.cs`
- Cards are multiplayer-only via `MultiplayerConstraint => CardMultiplayerConstraint.MultiplayerOnly`; for other kinds the only marker is an `IsAllowed`-style check on `Players.Count`, so the scan treats `return ... Players.Count > 1;` as multiplayer-only. (T4) `MegaCrit.Sts2.Core.Models.Cards/Cacophony.cs:16`, `MegaCrit.Sts2.Core.Models.Relics/MassiveScroll.cs:21`
- Overgrowth events can start extra encounters (`DenseVegetationEventEncounter`, `FakeMerchantEventEncounter`) whose monsters are not in `Overgrowth.GenerateAllEncounters`. (T4) `MegaCrit.Sts2.Core.Models.Events/`
- Referee only: the trace state of the step that ends a combat is taken after the room was torn down (piles empty, `enemies` empty, Osty `alive: false`, player block cleared, gold rewarded), not at the moment of the last effect. The differential harness therefore compares only `combat_over`, `player_dead`, `round`, `side` and player `hp`/`max_hp` on that step. Observed over 50 seeds in `tools/diff.sh`; not checked against the decomp. (D1) `crates/sts2-diff/src/compare.rs`
