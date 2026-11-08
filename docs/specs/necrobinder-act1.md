# Necrobinder and Act 1 content spec (R3)

Source: decomp under `ref/decomp/`; refs are `path:line`. Prefixes (all under `ref/decomp/MegaCrit.Sts2.Core.`):
`CH`=Models.Characters/Necrobinder.cs · `CHM`=Models/CharacterModel.cs · `PL`=Entities.Players/Player.cs · `PCS`=Entities.Players/PlayerCombatState.cs ·
`CR`=Entities.Creatures/Creature.cs · `CC`=Commands/CreatureCmd.cs · `OC`=Commands/OstyCmd.cs · `PC`=Commands/PlayerCmd.cs · `ATK`=Commands.Builders/AttackCommand.cs ·
`CS`=Combat/CombatState.cs · `HK`=Hooks/Hook.cs · `DOOM`=Models.Powers/DoomPower.cs · `ACT`=Models/ActModel.cs · `OG`=Models.Acts/Overgrowth.cs ·
`UD`=Models.Acts/Underdocks.cs · `ENC`=Models.Encounters/ · `MON`=Models.Monsters/ · `CARD`=Models.Cards/ · `REL`=Models.Relics/ · `POW`=Models.Powers/.
`R1 §x` = `docs/specs/combat-loop.md`. Ascension numbers: `AscensionLevel` enum order (`Entities.Ascension/AscensionLevel.cs`) gives SwarmingElites=1 … ToughEnemies=8,
DeadlyEnemies=9, DoubleBoss=10 (implicit values); `HasLevel(l)` is `_level >= l` (`Entities.Ascension/AscensionManager.cs:45`). Counts below come from
`INDEX.tsv` plus regex scripts, so treat them as ±2.

## 0. Summary
* Necrobinder: 66 HP, 99 gold, 3 energy, 5-card hand, 3 potion slots, starter relic **Bound Phylactery** (summons Osty).
* **Osty** is not a player and not an enemy: it is a pet `Creature` on the player side whose model is `Osty`. It has a no-op move, never takes a turn, and acts only when a card/power runs
  `AttackCommand.FromOsty`. It absorbs damage that got through the owner's block (`DieForYouPower`), and **dead Osty stays in combat** and can be revived by a later Summon.
* **Doom** is a counter debuff, not damage: a creature with `CurrentHp <= Doom` is killed at the end of its side's turn. **Souls** are just a generated 0-cost token card (`Soul`).
* Act 1 is **Overgrowth or Underdocks**, chosen with the `act_selection` rng at run start; each act has 4 weak, 10–12 normal, 3 elite and 3 boss encounters.
* Recommended first-milestone monster: `FuzzyWurmCrawlerWeak` (one monster, fully deterministic, only attack + Strength). Full spec in §5.

## 1. The Necrobinder character
| Item | Value | Ref |
|---|---|---|
| Max/starting HP | 66 | `CH:32` |
| Starting gold | 99 | `CH:34` |
| Max energy | 3 (not overridden; base value) | `CHM:97`, `PL:307` passes `character.MaxEnergy` |
| Orb slots | 0 | `CHM:101` |
| Potion slots | 3 (`initialMaxPotionSlotCount`); no starting potions | `PL:28`, `PL:307`, `CHM:115` |
| Hand draw | `ModifyHandDraw(5)`; hand cap 10 | R1 §1.3 step 8d, §3.4 |
| Card / relic / potion pool | `NecrobinderCardPool` (91 cards), `NecrobinderRelicPool` (8), `NecrobinderPotionPool` (3) | `CH:36-40` |
| Starter relic | `BoundPhylactery` | `CH:62` |

Starter deck (`CH:48-60`, in this order; deck order feeds the initial shuffle, R1 §1.1):
| Card | Cost | Type / target | Effect (base → upgraded) | Ref |
|---|---|---|---|---|
| StrikeNecrobinder ×4 | 1 | Attack, AnyEnemy, Basic | 6 damage → 9 (`UpgradeValueBy(3)`) | `CARD/StrikeNecrobinder.cs:19,25,32,39` |
| DefendNecrobinder ×4 | 1 | Skill, Self, Basic | 5 block → 8 | `CARD/DefendNecrobinder.cs:17,20,26,31` |
| Bodyguard ×1 | 1 | Skill, Self, Basic | `OstyCmd.Summon(5)` → 7 | `CARD/Bodyguard.cs:14,18,26,31` |
| Unleash ×1 | 1 | Attack, AnyEnemy, Basic, tag `OstyAttack` | Osty attacks for `6 + 1 × Osty.CurrentHp` → base 9 | `CARD/Unleash.cs:20-24,36-43,46-49` |

* **Unleash** damage: `CalculatedVar.Calculate` = `CalculationBase + ExtraDamage × multiplier` (`Localization.DynamicVars/CalculatedVar.cs:62-70`), multiplier = Osty's current HP if alive else 0, and 0
  when not in a live combat. It is computed per hit and passed to `CreatureCmd.Damage` with **dealer = Osty** (`ATK:669`), so the player's Strength/Weak do not apply (they require `Owner == dealer`, R1 §4.2),
  while the target's Vulnerable does. If Osty is dead/missing, `Osty.CheckMissingWithAnim` is true, the damage is skipped, but energy is still spent and the card still goes to its result pile (`CARD/Unleash.cs:39`).
* Damage-modifying powers keyed on Osty as dealer: `CalcifyPower` adds `+Amount` to powered attacks whose `dealer.Monster is Osty` and `dealer.PetOwner == Owner` (`POW/CalcifyPower.cs:15-26`).
* Run init: HP/gold/energy/slots are set once in `PL:307`; the starter deck and relic are added by run init (not read in this task, see §7).

### 1.1 Bound Phylactery (`REL/BoundPhylactery.cs`)
* Starter rarity, `SpawnsPets`, `SummonVar(1)` (`:14-18`).
* `BeforeCombatStart` (pass 1 of R1 §1.2 step 3) → `OstyCmd.Summon(ThrowingPlayerChoiceContext, Owner, 1, this)` (`:22-25,40-43`). So Osty exists with **1 HP / 1 max HP before turn 1**.
* `AfterEnergyResetLate(player)`: if `player == Owner && TurnNumber != 1` → `SummonPet()` again (`:32-38`). `Hook.AfterEnergyResetLate` runs inside `SetupPlayerTurn` step 8b, after the energy reset and
  *before* `BeforeHandDraw` (`HK:524`; R1 §1.3). Each later turn: Osty alive → **+1 max HP and +1 current HP**; Osty dead → **revive with max=current=1**.
  (Late pass deliberately lets "does Osty exist" checks, e.g. `Friendship`, run first, `:27-31`.)

## 2. Osty
### 2.1 What it is
* A `Creature` built by `PlayerCmd.AddPet<Osty>` (`PC:244-264`): `combatState.CreateCreature(Osty model, player.Creature.Side, slot=null)` then `PlayerCombatState.AddPetInternal` then `CreatureCmd.Add`.
  Side = Player. `PetOwner` is set once and may never change (`CR:181-196`); `IsPet => PetOwner != null` (`CR:200`). Osty lives in `PCS._pets` (`PCS:17,27,234-246`) and in `CombatState._allies`
  (listener order: player creature, then pets, R1 §2.3).
* `Player.Osty` = the pet even when **dead**; `IsOstyAlive`, `IsOstyMissing = !IsOstyAlive` (`PL:125-142`). All Osty cards test these.
* **Not a player creature**: `CombatState.PlayerCreatures` filters `c.IsPlayer` (`CS:76`), and monster attacks without an explicit target hit `PlayerCreatures` (`ATK:171-174`). So **monsters never choose Osty as a
  target**; Osty is only reached by redirection (§2.3). `IsHittable` for AoE uses `Hook.ShouldAllowHitting` (`CR:286-298`); `DieForYouPower.ShouldAllowHitting` returns `creature.IsAlive` (`POW/DieForYouPower.cs:35-38`).
* Model: `MinInitialHp = MaxInitialHp = 1` (`MON/Osty.cs:27-29`); one move `NOTHING_MOVE` that does nothing and loops to itself (`:37-42`). It is on the player side, so `ExecuteEnemyTurn` never runs it
  (R1 §1.6). The player-side `StartTurn` still iterates it as a "creature on the side" (`BeforeTurnStart`, `AfterTurnStart` → `ClearBlock`; R1 §1.3 steps 2, 6), but Osty never holds block (block lives on the owner, §2.3).
* Because Osty's `Side == Player`, `CreateCreature` does not roll a unique HP (`CS:239` only for `Side == Enemy`) and does not use `Rng.Niche`. It does consume the next `CombatId` (`CS:259-261`) and
  seeds its `monster.Rng` (`CS:245`); this matters only for the creature-id numbering of anything created after it.

### 2.2 Summon, grow, die, revive — `OstyCmd.Summon(ctx, summoner, amount, source)` (`OC:35-91`)
1. `amount = Hook.ModifySummonAmount(...)` (`OC:38`); if 0, return (no events). Only `SandpitPower` and `AbstractModel` mention Summon hooks in the model dirs; Necrobinder content has no modifier (`UNVERIFIED` role of `SandpitPower`).
2. **Osty alive** → `CreatureCmd.GainMaxHp(osty, amount)` (`OC:48-51`) = `SetMaxHp(max+amount)` then `Heal(that delta)` (`CC:838-851`, `CC:891-901`). Current HP rises by `amount`.
3. **Osty dead (pet exists)** → "revive": `AddPetInternal` (idempotent), then `SetMaxHp(osty, amount)` and `Heal(osty, amount, playAnim:true)` (`OC:54-61,77-78`). **Max HP is reset to `amount`**; growth is lost.
   `HealInternal` clamps to `MaxHp` and fires `Revived` if it was dead (`CR:478-486`; `Player` is null for a pet so no `ActivateHooks`). Then `Hook.AfterOstyRevived(combatState, osty)` (`OC:79-82`, `HK:880-886`).
4. **No Osty yet** → `AddPet<Osty>`, then `PowerCmd.Apply<DieForYouPower>(osty, 1)` (`OC:65,74`), then `SetMaxHp(osty, amount)` and `Heal(osty, amount)` (`OC:77-78`).
5. All paths: `History.Summoned(...)` then `Hook.AfterSummon(combatState, ctx, summoner, amount)` (`OC:88-89`; ctx may suspend). `SummonResult(osty, amount)` is returned.
* `Heal` early-outs only when `CombatManager.IsEnding && !creature.IsPlayer` (`CC:737`), so a Summon during an ending combat is a no-op for Osty.
* Cosmetic parts of `Summon` (SFX, tweens, `OstyScaleToSize`, `Cmd.Wait`) are ignored.

### 2.3 Damage and block with Osty (extends R1 §4.2 steps 3, 5, 6)
* **Block is the owner's.** `blockHolder = target.PetOwner?.Creature ?? target` (`CC:285`): damage aimed at Osty is blocked by the Necrobinder's block, and block spent is taken from the player.
* **Redirection** — `DieForYouPower.ModifyUnblockedDamageTarget(target, _, props, _)` (`POW/DieForYouPower.cs:15-30`): redirect to Osty iff `target == Owner.PetOwner.Creature` (the Necrobinder),
  Osty is alive, and `props.IsPoweredAttack()` (Move set, Unpowered clear). Called from `CC:289` via `Hook.ModifyUnblockedDamageTarget` (`HK:2056`) after the BeforeOsty `ModifyHpLost` phase and before AfterOsty.
  Not redirected: unpowered/HP-loss damage (Doom, Haunt, card self-damage), anything aimed at someone else.
* The redirected amount is the **unblocked** damage. Osty's HP floors at 0; overkill is re-run through `ModifyHpLost(AfterOsty)` on the original target and applied to it as a second `DamageResult`
  (R1 §4.2 step 6, `CC:302-311`). So 5 unblocked damage into a 3-HP Osty kills it and the player loses 2 HP.
* `WasTargetKilled` is computed from the redirected target, so Osty dying runs the normal kill pipeline for Osty (`CC:430`, `KillWithoutCheckingWinCondition` `CC:514`), after the whole `Damage` call.
* **Osty death**: `OnPetDied` (`PCS:281-292`) removes the pet from `_pets` only if `Hook.ShouldCreatureBeRemovedFromCombatAfterDeath` — and `DieForYouPower` votes **false** for Osty itself (`POW/DieForYouPower.cs:40-47`),
  and `ShouldPowerBeRemovedAfterOwnerDeath` is false (`:49-52`). Result: dead Osty keeps its slot, stays in `_allies`, keeps `DieForYou` as a hook listener, has `CurrentHp == 0`, and is what `Player.Osty` returns.
  `AfterDeath` and `AfterCurrentHpChanged(delta<0)` fire for it. `Osty` is **also killed when its owner dies** (`CC:579-581`).
* `AfterCurrentHpChanged(creature, delta)` fires for Osty HP loss (`CC:393-429`); `NecroMasteryPower` reacts: when Osty loses HP, deal `-delta × Amount` unblockable unpowered damage to all hittable enemies
  (`POW/NecroMasteryPower.cs:17-24`).
* **Osty as attacker**: `AttackCommand.FromOsty(osty, card, cardPlay)` sets `Attacker = osty`, `ModelSource = card`, `SourceType.Card` (`ATK:238-251`). Single-target cards call `.Targeting(target)`.
  Random targeting uses `(Attacker.Player ?? Attacker.PetOwner).RunState.Rng.CombatTargets` (`ATK:617`). `Hook.BeforeAttack/AfterAttack` see Osty as attacker; `BoneFlute` keys on that
  (`command.Attacker.Monster is Osty && PetOwner.Creature == Owner` → `GainBlock(2, Unpowered)`; `REL/BoneFlute.cs`). `SicEmPower` triggers `Summon` from `AfterDamageGiven` when Osty hit the Owner (`POW/SicEmPower.cs`).
* `UNVERIFIED`: whether any card gives Osty its own block, how `AllAllies`/`AnyAlly` cards (`LegionOfBone`, `GlimpseBeyond`, `Soulbound`, all multiplayer-only) treat Osty in single player (all 3 are `MultiplayerOnly`), and Osty's behaviour with debuffs
  (nothing in `DieForYouPower` blocks power application while alive).

## 3. Other unique mechanics
### 3.1 Doom (`POW/DoomPower.cs`)
* `PowerType.Debuff`, `PowerStackType.Counter` (amounts add via `PowerCmd.Apply` merge, R1 §2.5). **The amount never decays.**
* **Doomed** = `Owner.CurrentHp <= Amount` (`DOOM:144-147`). Not damage: no block, no `ModifyDamage`, no Intangible/Vulnerable involvement.
* **Timing** (hook, not a power tick):
  * Enemy owner: `BeforeSideTurnEnd(side == Enemy)` (`DOOM:60-66`), i.e. in `EndEnemyTurnInternal` after every enemy has acted (R1 §1.6). So a doomed enemy still gets its move that round, then dies.
  * Player-side owner: `AfterSideTurnEnd(side == Player)` (`DOOM:68-74`), after the hand flush (R1 §1.5 step 5).
  * `ShouldDoomTrigger` (`DOOM:76-99`): false if combat `IsOverOrEnding`; owner not in `participants`; owner dead; owner not doomed; or **owner is not the first doomed creature on its side**
    (`GetDoomedCreatures(side).First() != Owner`). So exactly one Doom power fires per side-turn-end and it kills *all* doomed creatures of that side together.
* `DoomKill(creatures)` (`DOOM:40-53`): for each creature in side order `CreatureCmd.Kill(creature)` (full kill pipeline incl. `ShouldDie` preventers, `BeforeDeath/AfterDeath`; R1 §4.5), then **once**
  `Hook.AfterDiedToDoom(combatState, creatures)` (`HK:496-510`, unguarded). Kills are not batched into one `Kill` list, so win/loss is still only checked at the next `CheckWinCondition` (R1 §4.5).
  `MonsterModel.ShouldDisappearFromDoom` and `OnDieToDoom` are visual only (`MON` `MonsterModel.cs:172,493`).
* `EndOfDays` applies Doom to every hittable enemy then calls `DoomPower.DoomKill(GetDoomedCreatures(HittableEnemies))` immediately (`CARD/EndOfDays.cs`).
* Doom cards (all via `PowerCmd.Apply<DoomPower>` unless noted): BlightStrike (8 dmg, then Doom = Σ `DamageResult.TotalDamage` = blocked + unblocked, `Entities.Creatures/DamageResult.cs:63`), Scourge (13 + draw 1),
  Deathbringer (21 Doom + 1 Weak, all hittable, cost 2), NegativePulse (5 block + 7 Doom all), NoEscape (Doom `10 + 5×floor(targetDoom/10)`), EndOfDays (29 Doom all, immediate kill, cost 3), Oblivion (applies
  `OblivionPower`: each card the Applier plays adds Doom = stored amount, removed at player side end), TimesUp (damage = target's Doom, cost 2), DeathsDoor (6 block, ×3 if Doom applied by owner this turn),
  Countdown (power: at each of owner's side-turn starts apply `Amount` Doom to a random hittable enemy, **`Rng.CombatTargets`**, `POW/CountdownPower.cs:21-33`), Shroud (power: gain `Amount` unpowered block whenever owner applies Doom),
  Neurosurge, ReaperForm (both powers; `UNVERIFIED` details), Underworld (multiplayer-only).
* Relics: `BookRepairKnife` (Uncommon) heals 3 per non-owner creature killed by Doom for which all powers say `ShouldOwnerDeathTriggerFatal` (`REL/BookRepairKnife.cs`); `UndyingSigil` (Shop) multiplies damage dealt by a
  doomed creature (`dealer.CurrentHp <= Doom`) to the owner by 0.5 (`REL/UndyingSigil.cs`).
* Epoch-gated cards: Scourge, Oblivion, Countdown (Necrobinder2Epoch).

### 3.2 Souls (`CARD/Soul.cs`, token card, `TokenCardPool`)
* `Soul`: cost 0, Skill, Self, Rarity Token, keyword Exhaust, `CardsVar(2)`: **draw 2** (3 upgraded) (`:14-19,44-52`). Plays through the normal card pipeline (R1 §3.2) and exhausts.
* There is no "Soul resource". Creators and where the cards go: CaptureSpirit (3 dmg unblockable-unpowered + 3 Souls to **Draw, random position**), GraveWarden (8 block + 1 Soul to Draw random), Reave (10 dmg + 1 Soul, upgraded
  Soul to Draw random), Severance (13 dmg + 3 Souls: Draw random / Discard / Hand), Dirge (X-cost: X Summons of 3 + X Souls to Draw random), Seance (transforms chosen draw-pile cards into Soul), FuneraryMask relic (3 Souls into the Draw
  pile at turn 1, before the hand draw, `REL/FuneraryMask.cs`), PotOfGhouls potion. All use `CardPileCmd.AddGeneratedCardsToCombat`; "random" position = `CardPilePosition.Random` (advances `Rng.Shuffle`, R1 §3.4/§6).
* Consumers: SoulStorm (damage `9 + 4 × #Soul in Exhaust pile`, `CARD/SoulStorm.cs:7-9`), DevourLifePower (each Soul *played* → Summon `Amount`), HauntPower (each Soul played → `Amount` unblockable-unpowered damage to a
  random hittable enemy via `Rng.CombatTargets`), SoulboundPower (multiplayer-only).
* The `Soul` created-card flow raises `AfterCardGeneratedForCombat` (`POW/SoulboundPower.cs:24`) — relevant only to multiplayer.

### 3.3 Other power-/card-level mechanics worth modelling early
* `Ethereal` (exhaust at turn end unless retained) and `Retain` are very common in this pool (R1 §1.5 steps 3c, 5). `Unplayable` (Eidolon). `CalculatedVar` damage (Unleash, SoulStorm, TimesUp, NoEscape).
* `SummonNextTurnPower`/`EnergyNextTurnPower` style delayed effects; `CardTag.OstyAttack` marks Osty attacks.

## 4. Act 1: selection and structure
### 4.1 Which Act 1
* Run start (`MegaCrit.Sts2.Core.Multiplayer.Game.Lobby/StartRunLobby.cs:470-472`): `rng = new Rng(StringHelper.GetDeterministicHashCode(seed), "act_selection")`;
  `list = ActModel.GetRandomList(rng, unlockState, isMultiplayer)`; then `list[0] = GetAct(Act1) ?? list[0]` (lobby override).
* `ModelDb.Acts` = Overgrowth, Underdocks, Hive, Glory (`Models/ModelDb.cs:~300-320`); `ActsByIndex` buckets by `Index` (`:323-343`). Index: Overgrowth 0 (`OG:49`), Underdocks 0 (`UD:44`), Hive 1, Glory 2.
  Only Act 1 has a real choice; acts 2 and 3 have one candidate each (they still go through `rng.NextItem`).
* `GetRandomList` (`ACT:538-565`), for each index in order: iterate that index's acts in `ModelDb.Acts` order; keep those with `IsUnlocked`; **if an unlocked act is non-default, not multiplayer, test mode off, and
  `!SaveManager.Instance.Progress.DiscoveredActs.Contains(id)` it is chosen immediately without touching the rng** (forced first sighting); otherwise `rng.NextItem(candidates)` (`Rng.NextItem` = `NextInt(0,n)` then `ElementAt`,
  `Random/Rng.cs:289-299`). Candidates for index 0 are `[Overgrowth, Underdocks]` in that order.
* `Overgrowth.IsDefault = true`, `IsUnlocked = true` (`OG:51,129-132`). `Underdocks.IsDefault = false`, unlocked only if `UnderdocksEpoch` is revealed (`UD:46,107-110`).
  **Engine input needed**: `UnlockState` (revealed epochs) and `DiscoveredActs`. Recommended default: everything unlocked and discovered, i.e. a uniform `NextInt(0,2)` (0 → Overgrowth, 1 → Underdocks).
* Rooms are generated at run setup (`RunManager.cs:743-766`, called from `SetUpNewSingleplayer` `:311/343`) with `RunState.Rng.UpFront`, for every act in order: first the shared-ancient shuffle and per-act `NextInt` subset split
  (acts 2+ only), then `act.GenerateRooms(UpFront, unlockState, isMultiplayer)` (`ACT:331-386`). Then, if `ShouldApplyTutorialModifications()` (true in standard non-test mode, `RunManager.cs:~775-792`),
  `ApplyDiscoveryOrderModifications`: forces the boss to the first boss in `BossDiscoveryOrder` the player has not yet seen (`ACT:404-415`), plus a fixed room order when `NumberOfRuns == 0` (`OG:110-127`).
  Both are `UnlockState` inputs; a fully-seen, `NumberOfRuns > 0` state disables them. `DoubleBoss` ascension (10) adds a second boss only on the last act (`RunManager.cs:758-763`).

### 4.2 `ActModel` parameters shared by both acts
| Parameter | Value | Ref |
|---|---|---|
| Weak encounters at start | `NumberOfWeakEncounters = 3` | `OG:45`, `UD:40` |
| Rooms | `BaseNumberOfRooms = 15` (14 in multiplayer); floors = rooms + 2 | `OG:47`, `UD:42`, `ACT:261-278` |
| Map grid | 7 columns × (rooms+1) rows, `Rng("act_{n}_map")`; boss at the bottom centre | `Map/StandardActMap.cs:19,89-94,111-113` |
| Map point counts | rests `NextGaussianInt(7,1,6,7)`; unknowns `NextGaussianInt(12,1,10,14)`; elites 5 (×1.6 at SwarmingElites = 8); shops 3 | `OG:134-139`, `UD:112-117`, `Map/MapPointTypeCounts.cs` |
| Fixed rows | last row Rest, 7th from the end Treasure (Elite if Warden mod), row 1 Monster, start = Ancient (Neow) | `StandardActMap.cs:236-305` |
| Gold per fight | Monster 10–20, Elite 35–45, Boss 100 (×Poverty factor at ascension 3) | `Models/EncounterModel.cs:64-98` |

Full map generation (path building, pruning, point-type assignment) is outside this brief; `UNVERIFIED` beyond the table. The `?` (Unknown) room outcome roll uses a different rng (`UnknownMapPoint`) and is not covered.

### 4.3 Encounter / event sequences generated per run (`ACT:331-386`)
* **Events**: `AllEvents ++ ModelDb.AllSharedEvents`, minus epoch-gated ones (Event1Epoch = TrashHeap, Event2Epoch = Reflections, Event3Epoch = ColorfulPhilosophers), `UnstableShuffle(rng)` into `RoomSet.events`.
  Each visit takes `events[eventsVisited % n]`, skipping events whose `IsAllowed(runState)` is false or that were already visited (`Rooms/RoomSet.cs:70,110-125`; `ACT:437-443`). `IsAllowed` per event: `UNVERIFIED`.
* **Normal encounters**: first 3 from a `GrabBag` of the weak encounters (weight 1 each, refilled when empty), then (rooms − 3 = 12) from a bag of the non-weak Monster encounters (refilled when empty).
  Each pick uses `AddWithoutRepeatingTags`: `GrabAndRemove(rng, e => !e.SharesTagsWith(last) && e != last)`, falling back to an unfiltered grab (`ACT:419-430`).
  `GrabBag.GrabIndex` draws `rng.NextDouble() × totalWeight` and **re-draws until the predicate holds** (rejection sampling; `Helpers/GrabBag.cs`), so rng consumption varies.
* **Elites**: 15 picks from the elite bag (refilled when empty), same tag rule. **Boss**: `rng.NextItem(AllBossEncounters)`. **Ancient**: `rng.NextItem(unlocked ancients ++ shared subset)` (Act 1 = Neow, once `NeowEpoch` revealed).
* The visit order is simply the generated list: `NextNormalEncounter = normalEncounters[normalEncountersVisited % count]` (`RoomSet.cs:72-84`); only Monster/Elite/Boss rooms advance it.
* Per fight: `EncounterModel.GenerateMonstersWithSlots` seeds the encounter rng `new Rng(Seed + TotalFloor + GetDeterministicHashCode(Id.Entry))` (`Models/EncounterModel.cs:254-266`).
  Only encounters that randomise composition use it (`SlimesWeak` picks 2 of 2 small + 1 medium slime, `ENC/SlimesWeak.cs`).

### 4.4 Overgrowth (`OG:71-97`, 22 encounters, 13 act events; `OG:28-43`)
| Type | Encounters (monsters) |
|---|---|
| Weak (4) | `FuzzyWurmCrawlerWeak` (FuzzyWurmCrawler), `NibbitsWeak` (Nibbit alone), `ShrinkerBeetleWeak` (ShrinkerBeetle), `SlimesWeak` (3 slimes: 2 small + 1 medium, random types) |
| Normal (12) | CubexConstruct, Flyconid, Fogmog, Inklets (3 Inklet), Mawler, NibbitsNormal (2 Nibbit), OvergrowthCrawlers (ShrinkerBeetle + FuzzyWurmCrawler), RubyRaiders (Axe/Assassin/Brute/Crossbow/Tracker), SlimesNormal (6 slimes), SlitheringStrangler (+Jaxfruit), SnappingJaxfruit (+Flyconid), VineShambler |
| Elite (3) | BygoneEffigy, Byrdonis, PhrogParasite |
| Boss (3) | Vantom, CeremonialBeast, TheKin (2 KinFollower + KinPriest) |
Events (13): AromaOfChaos, ByrdonisNest, DenseVegetation, JungleMazeAdventure, LuminousChoir, MorphicGrove, SapphireSeed, SunkenStatue, TabletOfTruth, UnrestSite, Wellspring, WhisperingHollow, WoodCarvings.
Distinct monster classes referenced by Overgrowth encounters: ~29.

### 4.5 Underdocks (`UD:66-91`, 20 encounters, 10 act events; `UD:26-38`)
| Type | Encounters (monsters) |
|---|---|
| Weak (4) | `CorpseSlugsWeak` (2 CorpseSlug), `SeapunkWeak` (Seapunk), `SludgeSpinnerWeak` (SludgeSpinner), `ToadpolesWeak` (2 Toadpole) |
| Normal (10) | CorpseSlugs (3), Cultists (CalcifiedCultist + DampCultist), FossilStalker, GremlinMerc (+FatGremlin, SneakyGremlin, GasBomb), HauntedShip, LivingFog, PunchConstruct, SeapunkNormal (Seapunk + CalcifiedCultist), SewerClam, TwoTailedRats (3) |
| Elite (3) | PhantasmalGardeners (4 gardeners), SkulkingColony, TerrorEel |
| Boss (3) | WaterfallGiant, SoulFysh, LagavulinMatriarch |
Events (10): AbyssalBaths, DrowningBeacon, EndlessConveyor, PunchOff, SpiralingWhirlpool, SunkenStatue (also in Overgrowth), SunkenTreasury, DoorsOfLightAndDark, TrashHeap (epoch-gated), WaterloggedScriptorium.
Distinct monster classes: ~22. Both acts additionally draw from the 18 shared events: BrainLeech, CrystalSphere, DollRoom, FakeMerchant, PotionCourier, RanwidTheElder, RelicTrader, RoomFullOfCheese, SelfHelpBook,
SlipperyBridge, StoneOfAllTime, Symbiote, TeaMaster, TheFutureOfPotions, TheLegendsWereTrue, ThisOrThat, WarHistorianRepy, WelcomeToWongos (`Models/ModelDb.cs:157-180`).

## 5. First-milestone monster: `FuzzyWurmCrawler` in `FuzzyWurmCrawlerWeak`
Why: single monster in the encounter (`ENC/FuzzyWurmCrawlerWeak.cs`, `GenerateMonsters` returns one entry, no encounter rng), the move machine has **no random branch** (so zero `MonsterAi` draws),
and the only effects are one single-hit attack and `Strength` on itself. Runners-up: `ShrinkerBeetle` (needs `ShrinkPower` on the player), `Seapunk` (multi-hit + block + Strength, still deterministic; a good second monster),
`SludgeSpinner` (random branch, Weak, Strength).

Spec (`MON/FuzzyWurmCrawler.cs`):
| Item | A0–7 | ToughEnemies (≥8) | DeadlyEnemies (≥9) |
|---|---|---|---|
| Max HP, rolled uniformly in [min, max] | 55–57 (`:29,31`) | 58–59 | same as 8 |
| `AcidGoopDamage` (`:33`) | 4 | 4 | 6 |
| Inhale | gain **7 Strength** on itself (`:88`) | same | same |
* HP roll: `CombatState.CreateCreature` → `SetUniqueMonsterHpValue` (`CS:239-241`, `CR:372-384`): candidate set `{min..max}` minus the max HP of other creatures on the side; if non-empty
  `rng.NextItem(set)` (one `NextInt(0,count)` on **`Rng.Niche`**), else `NextInt(min,max+1)`. With one monster the set is the full range in ascending order.
  Multiplayer scaling is skipped in single player (`CS:242`, `CR:386-393`). The HP is rolled before the monster's own `Rng` is seeded (`CS:245`).
* No powers at spawn, no `AfterAddedToRoom` override, no minion/slot logic. State: `_isPuffed` is set true in Inhale and false in Acid Goop; it is animation-only here (`ShouldShowMoveInBestiary` is UI-only, `:115-118`).
* Move state machine (`:48-60`), initial state = `FIRST_ACID_GOOP`:
  * `FIRST_ACID_GOOP` → attack `AcidGoopDamage` (`SingleAttackIntent`), follow-up `INHALE`.
  * `INHALE` → `StrengthPower +7` on self (`PowerCmd.Apply<StrengthPower>(Creature, 7, applier=Creature, null)` with a `ThrowingPlayerChoiceContext`), intent `BuffIntent`, follow-up `ACID_GOOP`.
  * `ACID_GOOP` → attack `AcidGoopDamage`, follow-up `FIRST_ACID_GOOP` (identical to it, just a different id).
  * Resulting cycle, turn by turn: **attack, inhale, attack, attack, inhale, attack, …** (period 3: A I A). Damage per hit = `AcidGoopDamage + Strength`: 4, —, 11, 11, —, 18, 18, —, 25 …
* Transition/rng: `MoveState.GetNextState` returns `FollowUpState.Id` and ignores the rng (`MonsterMoves.MonsterMoveStateMachine/MoveState.cs`), so `RollMove` consumes **no** `MonsterAi` draws for this monster.
  The first `RollMove` (combat start) does not advance because no move was performed yet (R1 §1.2 step 2, `MonsterMoveStateMachine.cs:53`); turn 1 intent = attack. Next move is rolled at the start of each player turn (R1 §1.3 step 5).
* Attack execution: `DamageCmd.Attack(dmg).FromMonster(this).Execute(null)` (`:77-80`); with no explicit target it hits `PlayerCreatures` (`ATK:171-174`), so Osty can only receive redirected damage (§2.3). Each hit runs the full
  Damage pipeline (R1 §4.2): Strength is added in the additive pass, the player's Weak ×0.75 / Vulnerable ×1.5 apply, then block, then Osty redirect.
* Per-ascension rewards: gold 10–20 (`EncounterModel.cs:64-98`, ×Poverty factor ≥ ascension 3); `UNVERIFIED` reward rng/stream for gold and card rewards (not read).

## 6. Rough content count for the Act 1 slice
| Content | Count | Notes |
|---|---|---|
| Necrobinder cards | **91** in pool: 4 Basic, 20 Common, 38 Uncommon, 27 Rare, 2 Ancient; 35 Attack / 36 Skill / 20 Power | `CardPools/NecrobinderCardPool.cs`; 5 are multiplayer-only (GlimpseBeyond, Underworld, Cacophony, Soulbound, LegionOfBone) → 86 single-player; 9 are epoch-gated (Scourge, Oblivion, Countdown, Afterlife, Sacrifice, Calcify, SculptingStrike, Veilpiercer, BansheesCry) |
| Generated / shared cards | 1 Soul token + Colorless pool 65 + status/curse pools | `TokenCardPool` 14 refs, `ColorlessCardPool` 65 refs; only those Act 1 content can create are needed |
| Necrobinder relics | 8: BoundPhylactery (starter), BoneFlute (Common), BookRepairKnife, FuneraryMask (Uncommon), BigHat, Bookmark, IvoryTile (Rare), UndyingSigil (Shop) | `RelicPools/NecrobinderRelicPool.cs` |
| Shared relics | 118 refs in `SharedRelicPool` (upper bound; how many Act 1 can actually drop depends on rarity rules, `UNVERIFIED`) | |
| Potions | 3 Necrobinder (PotionOfDoom, PotOfGhouls, BoneBrew; gated by `Necrobinder4Epoch`) + 45 shared | |
| Monsters | Overgrowth ~29 classes, Underdocks ~22 (incl. summoned/minion classes) | per-act, only one act per run |
| Encounters | Overgrowth 22 (4 weak/12 normal/3 elite/3 boss); Underdocks 20 (4/10/3/3) | |
| Events | Overgrowth 13 + 18 shared; Underdocks 10 + 18 shared (+ Neow ancient) | |
| Powers | 268 power classes game-wide; the Necrobinder cards apply ~30 Necrobinder-specific powers plus Doom/Weak/Vulnerable/Strength/EnergyNextTurn | |

Necrobinder card complexity (heuristic from `OnPlay` content: *hard* = mentions `Osty`/`Summon`/`CardSelectCmd`; *medium* = Power cards or any `PowerCmd`/Doom/Soul; *simple* = the rest):
* **Simple (17)**: BansheesCry, Bury, DeathMarch, DefendNecrobinder, Defile, DrainPower, Eidolon, Eradicate, Melancholy, Parse, PullFromBelow, Reap, Sow, StrikeNecrobinder, TheScythe, Undeath, Wisp.
  (Several still use `Ethereal`/`Retain`/`Exhaust`/X-cost/`CalculatedVar` and a few have conditional rules, e.g. BansheesCry cost reduction, Reap, TheScythe growth, so "simple" means no new engine concept.)
* **Medium (45)**: BlightStrike, BorrowedTime, Cacophony, Calcify, CallOfTheVoid, CaptureSpirit, Countdown, DanseMacabre, Deathbringer, DeathsDoor, Debilitate, Defy, Delay, Demesne, EndOfDays, EnfeeblingTouch, Fear,
  ForbiddenGrimoire, Friendship, GlimpseBeyond, GraveWarden, Hang, Haunt, Lethality, Misery, NegativePulse, Neurosurge, NoEscape, Oblivion, Pagestorm, Putrefy, ReaperForm, Reave, Scourge, SentryMode, Severance, SharedFate,
  Shroud, SleightOfFlesh, SoulStorm, Soulbound, SpiritOfAsh, TimesUp, Underworld, Veilpiercer. (Needs Doom/Weak/Vulnerable/Strength, ~30 power models, Soul token creation.)
* **Hard (29)**: Osty/Summon interaction or a player choice: Afterlife, Bodyguard, BoneShards, Cleanse, DevourLife, Dirge, Dredge, Fetch, Flatten, Graveblast, HighFive, Invoke, LegionOfBone, NecroMastery, Poke, Protector, PullAggro,
  Rattle, Reanimate, RightHandHand, Sacrifice, SculptingStrike, Seance, SicEm, Snap, Spur, Squeeze, Transfigure, Unleash.
Choice cards (hand/pile selection, R1 §5.3): Cleanse, Dredge, Graveblast, SculptingStrike, Seance, Snap, Transfigure (7).

### 6.1 Suggested staging (smallest first)
1. **M0**: Necrobinder, starter deck (Strike/Defend/Bodyguard/Unleash), Bound Phylactery, Osty (summon/grow/redirect/die/revive), one fight vs `FuzzyWurmCrawlerWeak` at ascension 0. Needs Strength, decimal damage, block, `CalculatedVar`.
2. **M1**: the other Overgrowth weak encounters (Seapunk-style multi-hit/Block, Shrink, random branch states) and the 17 simple + Osty basics.
3. **M2**: Doom (end-of-turn kill batching) and Weak/Vulnerable cards, then Souls.
4. Later: choice cards, relics/potions, events, map, Neow, second act of Act 1.

## 7. Gotchas for porters
* Osty **revival resets max HP to the summon amount** (`OC:77`), while a live Summon adds to max and current HP (`OC:50`). Do not model Osty as "max HP only grows".
* Dead Osty is a creature that still exists (`HP 0`, still an ally and listener); `Player.Osty` is non-null but `IsOstyAlive` is false. Redirect is gated on `Owner.IsDead` (`DieForYouPower.cs:21`).
* Osty redirection applies only to *powered attacks* hitting the Necrobinder after block; Osty never has its own block; monsters never target it.
* Unleash/Osty attacks use dealer = Osty: Strength/Weak of the player do not apply, Calcify does.
* Doom kills batch by side: only the first doomed creature's power fires and kills everybody doomed together at **end of the enemy turn** (after they act). Doom never decreases.
* `GetRandomList`'s "first sighting" and the tutorial boss/order overrides depend on save-file state (`DiscoveredActs`, `HasSeenEncounter`, `NumberOfRuns`). The engine must treat these as explicit run-config inputs.
* Card pool membership depends on revealed epochs (9 cards, 3 potions, 3 events, Underdocks, Neow). Same: an explicit `UnlockState` input.
* `GrabBag` and `Rng.NextItem` over a `HashSet<int>` rely on enumeration order; `HashSet<int>` enumerates in insertion order when ints were inserted ascending, which holds here (`UNVERIFIED` after `ExceptWith`; with one monster there are no removals).
* `UNVERIFIED` in this spec: starting-deck/relic insertion at run init (order of obtain hooks), Neow's options and rng use, per-event `IsAllowed`, reward generation, the `?`-room roll, `AbstractModel` hooks overridden by the 8 Necrobinder relics beyond those quoted,
  and ReaperForm/Neurosurge/Demesne/etc. power bodies. Each needs its own task before porting those cards.
