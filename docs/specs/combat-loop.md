# Combat execution model (R1)

Source: decomp under `ref/decomp/`, file refs are `path:line`. Prefixes (all under `ref/decomp/MegaCrit.Sts2.Core.`):
`CM`=Combat/CombatManager.cs · `CS`=Combat/CombatState.cs · `HK`=Hooks/Hook.cs · `CR`=Entities.Creatures/Creature.cs ·
`CC`=Commands/CreatureCmd.cs · `CPC`=Commands/CardPileCmd.cs · `CCM`=Commands/CardCmd.cs · `CSC`=Commands/CardSelectCmd.cs ·
`PWC`=Commands/PowerCmd.cs · `CARD`=Models/CardModel.cs · `AE`=GameActions/ActionExecutor.cs · `GA`=GameActions/GameAction.cs ·
`AQS`=GameActions.Multiplayer/ActionQueueSet.cs · `HPC`=GameActions.Multiplayer/HookPlayerChoiceContext.cs ·
`PCS`=Entities.Players/PlayerCombatState.cs · `ATK`=Commands.Builders/AttackCommand.cs · `RS`=Runs/RunState.cs.
Ignore everything cosmetic: `Cmd.Wait/CustomScaledWait`, `Vfx/Sfx/Think/Talk` cmds, `N*` nodes, tweens, `Task.Yield`, FTUE, saves.
Single-player only (multiplayer sync, `NetId`, `Branching*` cross-player branching are out of scope; `LocalContext.NetId` is always set).

## 0. One-paragraph summary
There are two cooperating async loops. (a) The **turn loop** (`CM:576 StartCombatInternal`) owns phases and fires boundary
hooks. (b) The **ActionExecutor** (`AE:123`) runs player-input `GameAction`s (play card, use potion, end turn, hook-choice
resume) one at a time. Everything *inside* a card/potion/monster move/hook is a plain C# `async`/`await` call chain: **nested,
depth-first, state mutated in place, no event queue**. A hook listener that calls a command that fires more hooks simply
recurses. A player choice is an `await` that parks the whole chain until the UI answers. Win/loss is *not* checked inline; it is
deferred to explicit "safe points" (`CheckWinCondition`).

## 1. Setup and turn structure

### 1.1 Combat setup — `CM:444 SetUpCombat`
1. New `CombatTurnState` (`CM:450`); `RoundNumber=1`, `CurrentSide=Player` are the `CombatState` defaults (`CS:157-158`).
2. Per player: `ResetCombatState` (fresh `PlayerCombatState`, `TurnNumber=1` `PCS:37`), then `PopulateCombatState`
   (`Player.cs:806`): clone every deck card into the Draw pile (deck order), then `RandomizeOrderInternal`
   (`Entities.Cards/CardPile.cs:69`): `UnstableShuffle(Rng.Shuffle)` then `Hook.ModifyShuffleOrder(isInitial:true)`.
3. Per creature `AddCreature` (`CM:1096`): `Monster.SetUpForCombat` (builds move state machine, `SpawnedThisTurn=true`
   `MonsterModel.cs:411`); enemies sorted by encounter slot order (`CS:496 SortEnemiesBySlotName`).
4. Card instances in combat are **clones** of deck cards (`DeckVersion` link). Run-level hooks also visit the *deck* copies (§2.4).

### 1.2 Combat start — `CM:576-626`
1. Unpause executor, drain queue, re-pause (`CM:579-581`).
2. For each creature in `state.Creatures` (allies then enemies): `AfterCreatureAdded` (`CM:1125`): monster `AfterAddedToRoom`; if
   enemy → `Monster.RollMove` (consumes **MonsterAi** rng; the machine ignores a 2nd roll before the first move was performed,
   `MonsterMoveStateMachine.cs:60`, so the roll in 1.3 step 5 is a no-op on turn 1).
3. `IsInProgress=true`, then `Hook.BeforeCombatStart`: pass 1 `BeforeCombatStart` over all run+combat listeners, pass 2
   `BeforeCombatStartLate` (`HK:311-322`).
4. `StartTurn(player side)`, then loop `AwaitTurnEndAndSwitchSides` / `StartTurn` forever while `IsLive` (`CM:610-625`).

### 1.3 `StartTurn` — `CM:688-851` (same function for both sides; `side = CurrentSide`)
Let `creatures` = all creatures on the side (or only extra-turn players, `CM:699-712`); `players` = all players if player side.
1. `PlayerTurnPhase=None` for all players.
2. `creature.BeforeTurnStart` for each: snapshots `power.AmountOnTurnStart = Amount` (`CR:678`).
3. `Hook.BeforeSideTurnStart(side, creatures)` — per-listener `HookPlayerChoiceContext` (`HK:1156`).
4. Player side only: phase=`Start`; `PlayerActionsDisabled=false`; clear ready sets; create the two TCS signals (`CM:723-733`).
5. Player side and not an extra turn: **every enemy `PrepareForNextTurn` → `RollMove`** (MonsterAi rng; `CM:739-745`, `CR:547`).
   So the move shown during a player turn is rolled at that turn's start, *after* `BeforeSideTurnStart`, *before* block clear.
6. For each creature: `AfterTurnStart(side)` → `ClearBlock` (`CR:686-733`): **skipped for a player whose `TurnNumber==1`**;
   otherwise `Hook.ShouldClearBlock` (preventers e.g. Barricade) else `Block=0`; if prevented → `Hook.AfterPreventingBlockClear`.
   Enemy block clears at the start of the *enemy* turn; player block at the start of the *player* turn.
7. For each creature: `Hook.AfterBlockCleared(creature)` — fires **even if block was not cleared / turn 1** (`CM:762-770`).
8. Player side, per player in order: `SetupPlayerTurn` (`CM:877`), run as a task so a pending choice there does not block step 9:
   a. `Hook.ShouldPlayerResetEnergy` ? `ResetEnergy` (`Energy=MaxEnergy`) : `AddMaxEnergyToCurrent` (`PCS:162,167`).
   b. `Hook.AfterEnergyReset`.
   c. `Hook.BeforeHandDraw` (pass `BeforeHandDraw`, pass `BeforeHandDrawLate`, `HK:600`).
   d. `handDraw = Hook.ModifyHandDraw(base 5)` (pass normal then `...Late`, `HK:1692`); `Hook.AfterModifyingHandDraw(modifiers)`.
   e. If `TurnNumber==1`: move `ShouldStartAtBottomOfDrawPile` enchanted cards to bottom, then every Innate card to top
      (in pile order, each `MoveToTopInternal`, so the *last* innate ends on top); `handDraw=max(handDraw, innateCount)`; cap at 10 (`CM:905-920`).
   f. `CardPileCmd.Draw(handDraw, fromHandDraw:true)` (§3.4).
   g. `Hook.AfterPlayerTurnStart`: 3 passes `...Early`, normal, `...Late` (`HK:894`).
9. `Hook.AfterSideTurnStart(side, creatures)`: pass normal, pass `...Late` (`HK:1175`).
10. Player side: orb queue `AfterTurnStart` (Necrobinder has no orbs; ignore). Then per living player `RunAutoPrePlayPhase`
    (`CM:859`): await setup task; phase=`AutoPrePlay`; `CheckForEmptyHand`; `Hook.AfterAutoPrePlayPhaseEntered`
    (3 passes Early/normal/Late, `HK:940`); phase=`Play`.
11. `CheckWinCondition`; if still in progress: unpause executor, queue state=PlayPhase (queued card plays now start),
    `IsEnemyTurnStarted=false`, `TurnStarted` event (`CM:827-835`).
12. Enemy side instead: `IsEnemyTurnStarted=true`; `CheckWinCondition`; `ExecuteEnemyTurn` (`CM:837-849`).
Dead players are auto-marked "ready to end turn" (`CM:802-813`).

### 1.4 Play phase
The turn loop parks at `await endTurnSignalSource.Task` (`CM:656`). Meanwhile the executor processes `PlayCardAction`,
`UsePotionAction`, `EndPlayerTurnAction` strictly one at a time in id order (`AQS:179 GetReadyAction`). After **every** finished action
the executor calls `CheckWinCondition` (`AE:157-171`; skipped only for `EndPlayerTurnAction` and `ReadyToBeginEnemyTurnAction`).

### 1.5 End of player turn (two phases) — `CM:635-686, 1437, 1515, 1707, 1746`
`EndPlayerTurnAction` (`EndPlayerTurnAction.cs:35`) only calls `PlayerCmd.EndTurn` → `SetReadyToEndTurn` (`CM:932`) which completes
the signal. Then in the turn loop:
1. If the signal came from inside a running player-driven action, await its completion (`CM:662-671`).
2. `AfterAllPlayersReadyToEndTurn` (`CM:1437`): queue state=`EndTurnPhaseOne` → **every still-waiting player-driven action in the queue is
   cancelled and new ones are rejected** (`AQS:331`, `ActionQueueSynchronizer.cs:130`); wait until queue empty or only
   non-player-driven (hook-choice) actions remain (`CM:1475`).
3. **Phase one** `EndPlayerTurnPhaseOneInternal` (`CM:1515`):
   a. per player: phase=`AutoPostPlay`; `Hook.AfterAutoPostPlayPhaseEntered`; then phase=`End` (`CM:1532-1551`).
   b. `Hook.BeforeSideTurnEnd(Player, players)`: pass `BeforeSideTurnEndVeryEarly`, pass `...Early`, pass `BeforeSideTurnEnd`
      (each listener gets its own `HookPlayerChoiceContext`; joined with `Task.WhenAll`, `HK:1244`). `CheckWinCondition`.
   c. per player `DoTurnEnd` (`CM:1599`): orb `BeforeTurnEnd`; abort if combat ending; scan hand (hand order): cards with
      `HasTurnEndInHandEffect` → list A; else Ethereal && `Hook.ShouldEtherealTrigger` → list B. Exhaust B in order
      (`CardCmd.Exhaust(causedByEthereal:true)`). Then for A *serialised in order*: move to Play pile → `OnTurnEndInHand` →
      Exhaust (if Ethereal) else Discard bottom (`CM:1633-1681`). `CheckWinCondition`.
   d. `Hook.BeforeFlush` per player: pass `BeforeFlush`, pass `BeforeFlushLate` (`HK:544`). `CheckWinCondition`.
4. Queue state=NotPlayPhase (player queues paused); enqueue `ReadyToBeginEnemyTurnAction` (not player-driven) → completes
   `beginEnemyTurnSignal` (`CM:1003-1029`, single-player fires immediately).
5. **Phase two** `EndPlayerTurnPhaseTwoInternal` (`CM:1746`): per player `FlushPlayerHand` (`CM:1785`):
   `Hook.ShouldFlush`; every non-`ShouldRetainThisTurn` hand card → `CardPileCmd.Add(cards, Discard)` as one batch (bottom;
   **no `AfterCardDiscarded`, no CombatHistory discard entry** — only `AfterCardChangedPiles`); `Hook.AfterFlush`;
   `PlayerCombatState.EndOfTurnCleanup()` (per card: clears `ExhaustOnNextPlay`, single-turn Retain/Sly, turn-scoped cost mods, `CARD:1611`).
   Then `Hook.AfterSideTurnEnd(Player, players)`: pass normal, pass `...Late` (`HK:1279`; each listener own context, joined per pass).
6. `SwitchFromPlayerToEnemySide` (`CM:1825`): collect `ShouldTakeExtraTurn` players; `SwitchSides`; `AfterTakingExtraTurn` hooks.
   `SwitchSides` (`CM:1857`): Player→Enemy sets `CurrentSide=Enemy`; Enemy→Player (or extra turn) sets Player, **`RoundNumber++`
   (not for extra turns) and `TurnNumber++` per player**; then `creature.OnSideSwitch()` for all (`SpawnedThisTurn=false` for monsters) and `TurnEnded` event.

### 1.6 Enemy turn — `CM:1402-1435`
`StartTurn(enemy side)` runs 1.3 steps 1,2,3,6,7,9,12 (no phases/draw/energy/roll). `ExecuteEnemyTurn`: for each enemy in a
**snapshot** of `Enemies` (slot order) still in the state: `Creature.TakeTurn` (`CR:711`) → `Monster.PerformMove` (skipped if
`SpawnedThisTurn`) → `await WaitForUnpause`, `CheckWinCondition`, return if ended. Then `EndEnemyTurn` (`CM:1073`):
`EndEnemyTurnInternal` = `Hook.BeforeSideTurnEnd(Enemy, enemies)` (3 passes), `EndOfTurnCleanup()` for every player, `Hook.AfterSideTurnEnd(Enemy, enemies)`
(`CM:1696`); `CheckWinCondition`; if not ending → `SwitchSides` (→ player turn, round++).
`PerformMove` (`MonsterModel.cs:435`): `IsPerformingMove=true`; run the move lambda against `PlayerCreatures`;
`MoveStateMachine.OnMovePerformed`; `History.MonsterPerformedMove`; remove the creature from `CombatState` if it died during its own move.
Next move is **not** rolled here; it is rolled at next player-turn start (1.3 step 5).
Duration debuffs (Weak/Vulnerable/Frail) tick down in their own `AfterSideTurnEnd` when `side==Enemy` (`WeakPower.cs`, `VulnerablePower.cs:60`);
Intangible `Decrement`s likewise; a debuff applied *to a player* sets `SkipNextDurationTick` (`PWC:148-150`) so it survives its first tick.

### 1.7 Combat end
See §4.5. `PlayerTurnPhase` enum order: None, Start, AutoPrePlay, Play, AutoPostPlay, End (`Combat/PlayerTurnPhase.cs`).

## 2. Execution model

### 2.1 Async/await vs queue — the answer
* **Effects are nested, depth-first `await` chains.** E.g. `StrikeNecrobinder.OnPlay` (`Models.Cards/StrikeNecrobinder.cs:29`) awaits
  `DamageCmd.Attack(...).Execute()` (`ATK:526`) → `CreatureCmd.Damage` (`CC:258`) → `Hook.*` loops → each listener's `await` body →
  more commands → more hooks. Every call completes (all of its hooks, in listener order, fully, recursively) before the caller continues.
  There is **no deferred/queued "action" for effects** (unlike STS1). Rust must preserve: *the continuation of a command runs only after all
  hooks triggered by it have finished, including hooks triggered by hooks.*
* `GameAction` (`GA:31`) is a thin wrapper used **only for player input** (class doc `GA:12-28`): `PlayCardAction`, `UsePotionAction`,
  `EndPlayerTurnAction`, `ReadyToBeginEnemyTurnAction`, `GenericHookGameAction` (a choice raised from a non-action context, §5.3).
  Queue = per-player list; next action = lowest `Id` among queue heads (`AQS:179-251`). One action executes at a time.
* The turn loop is *not* an action; it calls hooks directly and parks on TCS signals during the play phase.
* Monster moves run inside the turn loop (not actions), awaited sequentially; they use `BlockingPlayerChoiceContext` (no real choices).

### 2.2 Re-entrancy facts a port must reproduce
* Listener lists are **snapshots**: built eagerly on first `MoveNext` (`CS:411-486`, `RS:545-596`). Models added during dispatch are
  not visited (except `ModHelper` subscribers, ignore). But each snapshot entry is re-checked with `Contains(item)` *when it is about to
  be yielded* (`CS:480`, `RS:586`), so a model **removed** mid-dispatch (card left combat, power removed, creature removed) is skipped.
* A model that changed piles mid-dispatch still gets called if still contained anywhere.
* `Hook.IterateCombatHookListeners` (`HK:53`) yields **nothing** once `IsOverOrEnding && !IsStarting`, evaluated **once at the start of
  the dispatch** (not per listener). So a dispatch that began while combat was live runs all listeners even if one of them kills the last enemy.
  Hooks documented as "dispatched directly" bypass the guard (they are part of the kill sequence): `AfterBlockBroken`(`HK:107`),
  `AfterCardPlayed`/`Late`(`HK:278`), `AfterDamageGiven`(`HK:401`), `ShouldStopCombatFromEnding`(`HK:2450`), plus every hook taking `IRunState`
  (`AfterDamageReceived`, `BeforeDeath`, `AfterDeath`, `ShouldDie`, `AfterCurrentHpChanged`, `Modify*Damage/HpLost`, …) which never use the guard.
  Rule of thumb: **hooks taking only `ICombatState` are guarded; hooks taking `IRunState` are not** (`UNVERIFIED` for every one of the 149 — check per hook when porting).
* `IsCombatEnding` (`CM:417`) is *derived*: true as soon as no living *primary* enemy remains (and no `ShouldStopCombatFromEnding`), or a
  pending loss exists. `IsAlive = CurrentHp>0` (`CR:207`), so the guard goes dark the instant the last enemy's HP hits 0, before `Kill` runs.
* Commands early-out on `IsOverOrEnding` (`GainBlock` `CC:667`, `Draw` `CPC:1013`, `Exhaust` `CCM:239`, `AttackCommand` `ATK:533`, `CardPileCmd.Add` to combat pile when `IsEnding` `CPC:381`).
  The card-play multi-play loop `break`s on it (`CARD:1898`).

### 2.3 Listener order — `CombatState.IterateHookListeners` (`CS:411`) and `RunState.IterateHookListeners(combat)` (`RS:545`)
Combat listeners, in this exact order:
1. For each creature in `allies ++ enemies` (`CS:414`; allies = player creatures then pets like Osty in insertion order; enemies in slot order):
   a. `creature.Powers` in **application order** (`_powers` append, `CR:616`; stacking on an existing power keeps its slot).
   b. Enemy / pet: its `Monster` model (`CS:421`). Player: if `!IsActiveForHooks` (dead) skip the rest; else
      relics (skip melted) in acquisition order → potions by slot (non-null) → orbs → cards of piles **Hand, Draw, Discard, Exhaust, Play**
      (`PCS:70-80`), pile order, each card immediately followed by its Affliction then Enchantment.
2. Then combat `Modifiers`, `BadgeModels`, `MultiplayerScalingModel`.
Run-level listeners (`RS:545`) used when a hook takes `IRunState`: first, for each active player, **deck** cards (each followed by its
enchantment) — the *deck* copies, not the combat clones; then (only if no child combat) relics, potions, modifiers; then **the full combat list above**.
`UNVERIFIED`: whether any deck-copy card overrides a hook (the combat clone is the only one we know fires effects; check cards with `AfterDamageReceived` etc.).
Dead player: creature still listed (powers) but relics/potions/cards skipped; Osty is killed with its owner (`CC:579-581`).

### 2.4 Hook shape taxonomy (what a Rust dispatcher must distinguish)
* **Sync value hooks** — `Modify*`, `Should*`, `ModifyXxxLate`/`ShouldXxxLate` (`HK:1309-2510`). Pure functions of state, no `await`, can never
  suspend. Folded across listeners in listener order, threading the running value (`HK:1322 ModifyBlock`, `HK:2519 ModifyDamageInternal`). Predicates
  return on the first veto and report the `preventer` (`HK:2237 ShouldDie`: pass 1 `ShouldDie`, then pass 2 `ShouldDieLate`).
  A follow-up async "After modifying X" hook is then called **only for the models that changed the value** (`AfterModifyingBlockAmount`, `...DamageAmount`, …, `HK:661-851`).
* **Async notification hooks** `Before*/After*` — iterate listeners awaiting each (`HK:80-300…`). Those taking a `PlayerChoiceContext` can suspend (§5).
  Passes: `X` then `XLate` (full second iteration, not interleaved), some have `Early`/`VeryEarly` first. Known multi-pass hooks:
  `BeforeCombatStart(+Late)`, `BeforeHandDraw(+Late)`, `AfterPlayerTurnStart(Early,normal,Late)`, `AfterAutoPrePlayPhaseEntered(Early,normal,Late)`,
  `AfterSideTurnStart(+Late)`, `BeforeSideTurnEnd(VeryEarly,Early,normal)`, `AfterSideTurnEnd(+Late)`, `BeforeFlush(+Late)`, `AfterCardPlayed(+Late)`,
  `AfterCardChangedPiles(+Late)`, `AfterDamageReceived(+Late)`, `AfterEnergyReset`(+Late in model), `AfterCardDrawn(Early in model)`.
* **Fork-join turn-boundary hooks**: `BeforeSideTurnStart`, `BeforeSideTurnEnd`, `AfterSideTurnEnd`, `BeforeFlush` (but **not** `AfterFlush`, which is a plain sequential loop on one context, `HK:572`) give each listener a
  fresh `HookPlayerChoiceContext` and `await AssignTaskAndWaitForPauseOrCompletion` (`HPC:148`): the next listener starts once the previous one
  *completes or parks on a choice*; all are joined at the end with `Task.WhenAll`. With no choices this is exactly sequential. With a choice the later
  listeners' bodies run before the choosing one resumes (`UNVERIFIED` which real listeners hit this; Necrobinder set likely none — grep `PlayerChoiceContext` users of these hooks).
* Hooks also run `model.InvokeExecutionFinished()` after each listener — UI-only, ignore.

### 2.5 Power / model lifecycle commands
`PowerCmd.Apply` (`PWC:105`): early-out if ending / amount 0 / can't receive; **merge into existing instance** (`FindExistingInstanceForStacking`
`PWC:169`: `None`→same id, `InstancedPerApplier`→same applier, `Instanced`→never) via `ModifyAmount`; else:
`BeforePowerAmountChanged` → `ModifyPowerAmountGiven` (applier's listeners) → `ModifyPowerAmountReceived` → `power.BeforeApplied` →
`ApplyInternal` (append to `_powers`) → `History.PowerReceived` → set `SkipNextDurationTick` for debuffs on players → `AfterModifyingPowerAmountGiven/Received`
→ `power.AfterApplied` → `AfterPowerAmountChanged`. `ModifyAmount` (`PWC:219`) does `Amount += (int)modifiedOffset` (truncation) and `Remove` if `ShouldRemoveDueToAmount`
(`Remove` → `RemoveInternal`, `AfterRemoved`).

## 3. Card-play pipeline

### 3.1 Enqueue → execute
UI → `CardModel.TryManualPlay` (`CARD:1787`) → `ActionQueueSynchronizer.RequestEnqueue(new PlayCardAction(card, target))`. `CombatPlayPhaseOnly` actions
requested outside the play phase are deferred until the next PlayPhase (`ActionQueueSynchronizer.cs:149`). `PlayCardAction.ExecuteAction` (`PlayCardAction.cs:62-104`):
1. Resolve target by combat id. If card is **no longer in Hand → silent return (no cancel)**. If `!CanPlay() || !IsValidTarget` → `Cancel()` and nothing is spent.
   `CanPlay` (`CARD:1725`): not `Unplayable` keyword; `HasEnoughResourcesFor` (energy/stars); AnyAlly needs ≥2 living players; `Hook.ShouldPlay(AutoPlayType.None)` (guarded); `IsPlayable` card logic.
2. `SpendResources` (`CARD:1807`): `energy = EnergyCost.GetAmountToSpend()`, stars = `max(0, GetStarCostWithModifiers())`; if `ShouldPayExcessEnergyCostWithStars` the shortfall costs 2 stars each.
   `SpendEnergy`: X-cost captures `CapturedXValue = amount`; if `amount>0` `History.EnergySpent` then `LoseEnergy`; **always** `Hook.AfterEnergySpent(card, amount)` (even for 0). `SpendStars`: `AfterStarsSpent` only if >0.
3. `new GameActionPlayerChoiceContext(this)` then `card.OnPlayWrapper(ctx, target, isAutoPlay:false, resources)`.

### 3.2 `CardModel.OnPlayWrapper` — `CARD:1858-2019` (autoplay `CardCmd.AutoPlay` `CCM:51` enters here too, with `isAutoPlay:true`)
1. Manual: `CardPileCmd.AddDuringManualCardPlay` (`CPC:830`): remove from hand, append to **Play pile**, fire `Hook.AfterCardChangedPiles(old=Hand)` only (no `AfterCardEnteredCombat`).
   Auto: `CardPileCmd.Add(card, Play, Bottom)` (`CPC:373`).
2. **Result pile decided up front**: `GetResultLocationForCardPlay` (`CARD:2084`): `IsDupe || Type==Power` → `None` (removed from combat); `ExhaustOnNextPlay` (**consumed here**) or `Exhaust`
   keyword → Exhaust; else Discard (bottom). Then `Hook.ModifyCardPlayResultLocation` (sync fold, `HK:1403`) and `AfterModifyingCardPlayResultLocation` on changers.
3. `playCount = enchant replay count + 1`, `Hook.ModifyCardPlayCount` fold, `AfterModifyingCardPlayCount` (`CARD:2029`). Return if owner dead.
4. `CombatManager.BeginCardOrPotionEffect(owner)`: per-player effect depth++ (`CM:385`). **Always paired** with `EndCardOrPotionEffect` in `finally`: depth--, and at 0 if owner is dead → `RemoveDeadPlayerCardsFromCombat` (`CM:396`).
5. For `i in 0..playCount`: break if `IsOverOrEnding`; build `CardPlay{Card,Player,Target,ResultPile,Resources,IsAutoPlay,PlayIndex,PlayCount}`;
   a. `Hook.BeforeCardPlayed(cardPlay)` (guarded);
   b. `History.CardPlayStarted`;
   c. `card.OnPlay(ctx, cardPlay)` — the card-specific body (§3.3);
   d. return if owner dead; `Enchantment.OnPlay`; `Affliction.OnPlay` (each followed by owner-dead check);
   e. `History.CardPlayFinished`;
   f. if `IsInProgress`: `Hook.AfterCardPlayed` pass then `AfterCardPlayedLate` pass (unguarded, runs even when this play just killed the last enemy; `HK:278`).
6. `finally` effect depth--.
7. If still the same combat: owner change → `GiveToAnotherPlayer`; if the card is still in the Play pile: `None` → `RemoveFromCombat`; `Exhaust` → `CardCmd.Exhaust` (add to Exhaust pile, `History.CardExhausted`, `Hook.AfterCardExhausted`);
   else `CardPileCmd.Add(card, resultPile, position)` → hooks `AfterCardChangedPiles(+Late)`. **The card stays in the Play pile for the entire effect + `AfterCardPlayed` hooks** and moves *after* them.
   Note a plain move to Discard is `CardPileCmd.Add`, **not** `CardCmd.Discard`: no `AfterCardDiscarded` for played cards.
8. `CheckForEmptyHand` (`CM:1171`): if in progress, owner has no effect depth, and hand empty → `Hook.AfterHandEmptied`. (Deliberately deferred to here to avoid double triggers; also called after potion use and after auto-pre-play entry.)
9. Cleanup: `EnergyCost.AfterCardPlayedCleanup`, drop "until played" temp star costs; `Played` event; pop model.
10. Back in the executor: `CheckWinCondition` (`AE:170`).

### 3.3 Worked examples (`OnPlay` bodies)
* **StrikeNecrobinder** (`Models.Cards/StrikeNecrobinder.cs:29`): cost 1, `AnyEnemy`, damage var 6 (+3 upgraded), `ValueProp.Move`:
  `DamageCmd.Attack(6).FromCard(this, cardPlay).Targeting(target).Execute(ctx)` → §4.1.
* **DefendNecrobinder** (`...DefendNecrobinder.cs:24`): cost 1, `Self`: `CreatureCmd.GainBlock(Owner.Creature, DynamicVars.Block, cardPlay)` → §4.3. Base 5 (+3).
* **Bodyguard** (`...Bodyguard.cs:23`): cost 1, `Self`, `SummonVar(5)` (+2): `OstyCmd.Summon(ctx, Owner, 5, this)` (`OstyCmd.cs:35`): `Hook.ModifySummonAmount`;
  if Osty alive → `CreatureCmd.GainMaxHp(osty, amount)`; else revive existing or `PlayerCmd.AddPet<Osty>` + `PowerCmd.Apply<DieForYouPower>(osty, 1)`, then `SetMaxHp`, `Heal`, `AfterOstyRevived`; later `History.Summoned` + `Hook.AfterSummon(ctx, …)` (guarded; can suspend).
* **Dredge** (`...Dredge.cs:23`, choice): `n = min(Cards, 10 - handCount)`; if `n>0` → `CardSelectCmd.FromCombatPile(ctx, DiscardPile, Owner, prefs(n))` → `CardPileCmd.Add(selected, Hand)`.
* **Seance** (`...Seance.cs:26`, choice): `FromCombatPile(ctx, DrawPile, …, prefs(Cards))` then `CardCmd.TransformTo<Soul>(each)` — choice kind §5.

### 3.4 Draw / discard / exhaust / shuffle commands
* `CardPileCmd.Draw(count, fromHandDraw)` → `DrawInternal` (`CPC:1010`): early-out `IsOverOrEnding`; `Hook.ShouldDraw` veto (+`AfterPreventingDraw`); `n = Ceiling(count)` (decimal);
  hand room = `10 - hand.Count` (0 → return). Loop per card: stop if no room / combat ending; `ShuffleIfNecessary` (draw empty && discard non-empty → `Shuffle`); take draw-pile **first** card
  (top = index 0); `CardPileCmd.Add(card, hand)` (→ `AfterCardChangedPiles`); `History.CardDrawn`; `Hook.AfterCardDrawn` (listeners may autoplay → recursion; `Early` variant exists in model); room recomputed each iteration.
* `Shuffle` (`CPC:1076`): list = discard (pile order) ++ draw pile cards; `StableShuffle(Rng.Shuffle)`; `Hook.ModifyShuffleOrder(isInitial:false)`; added to Draw pile bottom in that order via one `Add` batch (bypasses hand-full redirect, fires `AfterCardChangedPiles`); then `Hook.AfterShuffle`.
* `CardPileCmd.Add` (`CPC:373-545`): validity → remove from old pile → insert (`Bottom`=-1, `Top`=0, `Random`=`Rng.Shuffle.NextInt(count+1)` `CPC:509`); **adding to a full hand (10) redirects the card to Discard** (`CPC:484`);
  `AfterCardEnteredCombat` only for cards with no old pile (`CPC:514`); finally per card `AfterCardChangedPiles(old pile type)` if the pile type changed (`CPC:535-541`).
* `CardCmd.Discard`/`DiscardAndDraw` (`CCM:172`): per card in order: `Add(discard pile)`, `History.CardDiscarded`, `Hook.AfterCardDiscarded`; **then** the draw; **then** autoplay of cards that were `IsSlyThisTurn` (`CCM:195-205`).
* `CardCmd.Exhaust` (`CCM:237`): one card at a time (bulk deliberately absent): `Add(Exhaust)`, `History.CardExhausted`, `Hook.AfterCardExhausted(causedByEthereal)`.
* `CardCmd.AutoPlay` (`CCM:51`): early-outs; Unplayable/`ShouldPlay` veto → move to result pile without playing; null target on AnyEnemy → `Rng.CombatTargets.NextItem(HittableEnemies)` (`CCM:77`); no target → card to result pile; X-capture; ensure in Play pile; `BeforeCardAutoPlayed`; `OnPlayWrapper(auto)`. Resources spent = 0.

### 3.5 Potion use (`UsePotionAction.cs:~100-121`, `PotionModel.cs:297`)
Remove potion from slot first (`RemoveBeforeUse`) → `Hook.BeforePotionUsed` → effect depth++ → `OnUse` → depth-- → `History.PotionUsed` → `Hook.AfterPotionUsed` → `CheckForEmptyHand`. Executor then `CheckWinCondition`.

## 4. Damage, block, death, combat end

### 4.1 Attack → damage (`ATK:526`, `CC:258`)
`AttackCommand.Execute`: bail if ending/attacker dead; `Hook.BeforeAttack`; `hits = Hook.ModifyAttackHitCount(base)`; per hit: stop if attacker dead or no living targets;
random-target attacks pick `Rng.CombatTargets.NextItem(validTargets)` **per hit** (`ATK:617`; "no duplicates" mode filters previous receivers); then **one full `CreatureCmd.Damage` per hit** (`ATK:669`,
so all hooks below run per hit); after all hits `History.CreatureAttacked`, `Hook.AfterAttack` (guarded).

### 4.2 `CreatureCmd.Damage` per target (`CC:276-392`), exact order
Skipped if dealer dead (returns empty results) or target already dead. For each target **sequentially**:
1. `modified = Hook.ModifyDamage(All)` (`CC:282`, `HK:1495→2519`). Order: card **enchantment** additive then multiplicative (`HK:1500-1510`);
   then over *all* listeners (run-level list): pass **Additive** (`ModifyDamageAdditive`, sum), pass **Multiplicative** (`ModifyDamageMultiplicative`, running product, one listener at a time),
   pass **Cap** (min of `ModifyDamageCap`, applied only if lower than running value). Final `max(0, x)`. Examples:
   Strength = `+Amount` iff `Owner==dealer` && powered attack (`StrengthPower.cs:16`); Vulnerable = `×1.5` iff `target==Owner` && powered attack, with
   multiplier modifiers from PaperPhrog/Cruelty/Debilitate (`VulnerablePower.cs:28`); Weak = `×0.75` iff `dealer==Owner` && powered (`WeakPower.cs:25`, Paper Krane / Debilitate modify);
   Intangible cap = 1 (`IntangiblePower.cs:47`). "Powered" = `Move` set and `Unpowered` clear (`ValueProps/ValuePropExtensions.cs:5`).
   **Order of multipliers = listener order (§2.3), applied sequentially in `decimal`**, so Weak·Vuln order can matter for rounding only through intermediate decimals (no rounding happens between them).
2. `Hook.AfterModifyingDamageAmount(modifiers)`; `Hook.BeforeDamageReceived` (run-level, unguarded) — receives the amount by value, cannot change it.
3. **Block is taken from the pet's owner**: `blockHolder = target.PetOwner?.Creature ?? target` (`CC:285`) — hitting Osty spends the Necrobinder's block. `blocked = holder.DamageBlockInternal(modified)` =
   `min(Block, amount)` (0 if `Unblockable`) and `Block -= (int)blocked` (`CR:431`).
4. `unblocked = Hook.ModifyHpLost(…, max(modified - blocked, 0), phase BeforeOsty)` (pass `ModifyHpLostBeforeOsty`, pass `...Late`, `HK:1725`); `AfterModifyingHpLostBeforeOsty`.
5. `newTarget = Hook.ModifyUnblockedDamageTarget` (Osty redirection, `HK:2056`); `unblocked = ModifyHpLost(… newTarget … phase AfterOsty)` (`ModifyHpLostAfterOsty`, `...Late`; Intangible clamps here: if ≥1 → 1, `IntangiblePower.cs:19`); `AfterModifyingHpLostAfterOsty`.
6. `result = newTarget.LoseHpInternal(unblocked)` (`CR:446`): `lost = (int)clamp(unblocked, 0, 999999999)`; `Hp = max(Hp - lost, 0)`; `WasTargetKilled = Hp>0 && amount >= Hp` (computed from the **decimal** amount before truncation);
   `Overkill = max(lost - hpBefore, 0)`. If redirected (`newTarget != target`): overkill is re-run through `ModifyHpLost(AfterOsty)` on the original target and applied to it as a second result (`CC:302-311`).
7. Flags: `WasBlockBroken = target.Block<=0 && blocked>0`; `WasFullyBlocked = !Unblockable && (blocked>0 || target.Block>0) && (int)unblocked==0` (`CC:294-295`).
8. `History.DamageReceived` per result (if in progress and not ending); VFX skipped.
After the target loop, for each result in order (`CC:393-429`): `AfterBlockBroken` (if broken, **unguarded**), `AfterCurrentHpChanged(-unblockedDamage)` if >0,
`AfterDamageGiven` (unguarded, `HK:401`), then `AfterDamageReceived`(+`Late`) **unless** the target was killed (`WasTargetKilled && IsDead` → queued for kill).
Finally `Kill(killedCreatures)` (`CC:430`) — **kills are processed after all hits/targets of this one Damage call**, not per target.

### 4.3 Rounding and the decimal type (everything is C# `decimal`, 28-29 digits, base 10)
* No `Math.Round/Floor` anywhere on the path. Values stay exact decimals (`7*1.5*0.75 = 7.875`) until an **`(int)` cast, which truncates toward zero**:
  HP loss `(int)Math.Clamp(amount,0,999999999)` (`CR:450`); block removal `Block -= (int)min(Block,amount)` (`CR:433-434`); block gain `Block = (int)min(Block+amount, 999999999)` (`CR:466`);
  power amount `Amount + (int)offset` (`PWC:240`); draw count `(int)Math.Ceiling(count)` (`CPC:1028`); `decimal.Truncate` for modifier-detection in `ModifyHpLost` (`HK:1735-1744`).
* Consequence (port exactly): damage `5.25` into `Block 10` → block `-5` (the `.25` vanishes), `unblocked=0`; damage `5.25` with `Block 3` → `blocked=3`, `unblocked=2.25` → loses 2 HP.
  `WasFullyBlocked` uses `(int)unblocked==0`, so 0.9 leaked damage still counts as fully blocked and loses 0 HP.
* Multiplications are applied one factor at a time in listener order (`HK:2535-2545`), additions one at a time; decimals are not combined or reordered.

### 4.4 Block gain / loss
`CreatureCmd.GainBlock(creature, amount, props, cardPlay)` (`CC:665`): return 0 if `IsOverOrEnding` or dead; `Hook.BeforeBlockGained`; `modified = Hook.ModifyBlock` (`HK:1322`):
card enchantment add then mul; then pass **additive** over listeners (Dexterity `+Amount` iff the card's owner/target is the Owner and powered `DexterityPower.cs:20`), then pass
**multiplicative** (Frail `×0.75` iff `target==Owner` and powered `FrailPower.cs:22`); `max(0, x)`; `AfterModifyingBlockAmount(modifiers)`; if `>0`: `GainBlockInternal` (truncating, §4.3), `History.BlockGained((int)amount)`;
`AfterBlockGained` (guarded, called even when the amount is 0). `LoseBlock(amount)` (`CC:710`) fires `AfterBlockBroken` only when block goes from >0 to 0.

### 4.5 Death, and when combat ends
* **Per kill** (`CC:514 KillWithoutCheckingWinCondition`): set HP to 0 if >0 (`AfterCurrentHpChanged`) → `Hook.BeforeDeath` → `Hook.ShouldDie` (2 passes; `ShouldDie`, `ShouldDieLate`; preventer e.g. Fairy). If dies:
  `Died` event; `Hook.AfterDeath`; remove from `CombatManager`/`CombatState` unless a monster mid-`PerformMove`; remove all powers (`RemoveAllPowersAfterDeath`) calling `AfterRemoved` each; for a primary enemy whose remaining teammates are all
  secondary → `Kill(teammates)`; for a player → clear orbs, kill Osty, `DeactivateHooks`, `HandlePlayerDeath` (`CM:1218`; remove cards unless mid-effect, energy/stars→0).
  If prevented: `AfterDeath(wasRemovalPrevented)`, `AfterPreventingDeath`, retry if dead again (max recursion 10).
* After `Kill` batch (`CC:471-507`): all players dead → `CombatManager.LoseCombat()` (sets `PendingLoss` only); else if a player died on the player side → `PlayerCmd.EndTurn(player)`.
* **Win/loss is evaluated only in `CheckWinCondition`** (`CM:1387`): pending loss → `ProcessPendingLoss` (`IsInProgress=false`, `CombatEnded`; **no `AfterCombatEnd` hooks on loss**); else `IsCombatEnding` → `EndCombatInternal`.
  Call sites: after every GameAction (`AE:170`), turn start (`CM:827,844`), after each enemy `TakeTurn` (`CM:1426`), after every end-turn phase-one step (`CM:1557,1576,1588`), enemy end of turn (`CM:1086`), plus the `IsOverOrEnding` early-outs in commands.
  Therefore remaining effects of a lethal card (rest of `OnPlay`, `AfterCardPlayed`, result-pile move, `CheckForEmptyHand`) still run; only guarded hooks and `IsOverOrEnding`-guarded commands become no-ops.
* `EndCombatInternal` (`CM:1297`): `IsInProgress=false`; clear extra-turn list; phases None; revive dead players (MP only); `Hook.AfterCombatEnd` (run+combat listeners — last hook that sees combat powers/piles);
  `History.Clear()`; `room.OnCombatEnded`; per player `AfterCombatEnd` (`Player.cs:837`: remove all powers without callbacks, clear piles, `LoseBlockInternal(all)`); `Hook.AfterCombatVictory`; saves/rewards; `CombatWon`/`CombatEnded` events.
  Player HP persists; piles/powers/block are discarded.
* Secondary enemies (Minion/Illusion powers) do not keep combat alive (`CR:253-279`); `Hook.ShouldStopCombatFromEnding` can veto (e.g. spawners).

## 5. Player choices: suspend and resume

### 5.1 Mechanism
Every effect receives a `PlayerChoiceContext`. A choice is raised by `CardSelectCmd.*`: `ReserveChoiceId` → `await ctx.SignalPlayerChoiceBegun(player, options)` → UI/selector →
`SyncLocalChoice` → `await ctx.SignalPlayerChoiceEnded()` (`CSC:252-316` pattern). Which context decides what "pause" means:
* **`GameActionPlayerChoiceContext`** (card plays, potions; `GameActionPlayerChoiceContext.cs:34`): `ActionQueueSet.PauseActionForPlayerChoice` (`AQS:261`) → action state `GatheringPlayerChoice`
  (`GA:188`); the `await` inside the effect simply never returns. Executor treats the action as paused and looks for another ready action, but a paused action **stays at the head of its queue**, which blocks that player's queue
  (`AQS:223`) — in single-player nothing else runs. `PlayerChoiceOptions.CancelPlayCardActions` (hand-selection choices) additionally cancels queued-but-unstarted `PlayCardAction`s.
  `SignalPlayerChoiceEnded` → `RequestResumeActionAfterPlayerChoice` → state `ReadyToResumeExecuting` with a **new action id** (`GA:168`) → executor re-runs `Execute` → completes the pending TCS (`GA:123-135, 180`) → the effect continues *exactly where it awaited*.
* **`HookPlayerChoiceContext`** (turn-loop hooks, listener hooks started from the turn loop): no `GameAction` exists until a choice is actually raised; then `GenerateHookAction` + enqueue a `GenericHookGameAction` (`HPC:77-108`) whose body awaits the hook's task (`GenericHookGameAction.cs:54`). Turn loop awaits `AssignTaskAndWaitForPauseOrCompletion`.
* **`BranchingPlayerChoiceContext`** wraps the card's context; for the owner's own choices it forwards to the original context (the only SP case) (`BranchingPlayerChoiceContext.cs:77`).
* `BlockingPlayerChoiceContext` (monster attacks, enemy-turn hooks): signals are no-ops; `ThrowingPlayerChoiceContext`: any choice throws (events, `PowerCmd.Decrement`).
* `PlayerChoiceContext.PushModel/PopModel` keep a stack of "which model is asking" (display only).

### 5.2 Auto-resolution rules (affect whether a decision point exists at all)
The pause is signalled *before* the options are filtered, but the selection is skipped when:
* option list is empty → empty result (`CSC:479`); `!RequireManualConfirmation && count <= MinSelect` → returns **all** options without asking (`CSC:483` pile, `407` grid, `840` hand). `RequireManualConfirmation = MinSelect>=0 && MinSelect != MaxSelect`
  (`CardSelectorPrefs.cs`: `(prompt, n)` = exact n; `(prompt, min, max)` = range). So "choose exactly 1 of 1" and "choose up to X" differ.
* `FromHandForUpgrade` auto-picks if ≤1 upgradable card (`CSC:927`).
* `FromHand`/`FromCombatPile`/`FromSimpleGrid` early-return empty when combat `IsEnding`/`IsOverOrEnding` before signalling (`CSC:390,451,819,910`).
* If `CardSelectCmd.Selector` (test/AI seam, `CSC:128`) is set, **no** signal is sent and `GetSelectedCards(options, min, max)` is called inline — the natural hook for an engine API.
* Pile selection presents Draw pile sorted by rarity then id but returns card identities (`CSC:492,504`), so the decision must carry card identity (UI order is irrelevant to the engine).
* `FromHand` returns cards in the order the screen selected them (`UNVERIFIED` whether order matters to callers; check Discard-N cards).

### 5.3 Choice kinds found (combat-relevant)
| Kind | API (`CSC` line) | Chooser sees | Result | Used by |
|---|---|---|---|---|
| Card target (pre-play) | `PlayCardAction.TargetId`, `CARD:1762 IsValidTarget` | living creatures valid for `TargetType` | creature id | every `AnyEnemy`/`AnyAlly` card |
| Potion target | `UsePotionAction.cs` | creature / none | creature id | targeted potions |
| Hand select (n or range) | `FromHand` 817, `FromHandForDiscard` 883, `FromHandForUpgrade` 908 | hand cards passing `filter` | ordered card set (1 for upgrade) | 23+10+1 card/relic/potion uses (Snap, SculptingStrike, Transfigure, …) |
| Combat-pile select | `FromCombatPile` 444/449 (draw/discard/exhaust) | pile cards passing `filter` | card set | 18 uses (Dredge, Seance, Cleanse, Graveblast, …) |
| Choose-a-card (≤3 canonical/generated) | `FromChooseACardScreen` 252 | ≤3 cards, optional skip | one card or none (`FromIndex`) | 13 uses (Discovery-style, Attack Potion) |
| Simple grid | `FromSimpleGrid` 388 | arbitrary list | index set | 1 use |
| Bundle choice | `FromChooseABundleScreen` 965 | list of card bundles | bundle | 1 use (UNVERIFIED if in combat) |
| Deck-based (upgrade/remove/transform/enchant/generic) | `FromDeckFor*` 537-754 | run deck | cards | mostly non-combat; reachable from in-combat cards (UNVERIFIED) |
| Relic / card-reward / rest-site / map | `RelicSelectCmd`, `CardReward`, `MendRestSiteOption` | — | — | out of combat |
`PlayerChoiceType` = None, CanonicalCard, CombatCard, DeckCard, MutableCard, Player, Index (`Entities.Models/PlayerChoiceType.cs`). No energy-X choice or yes/no prompt exists in combat: **X-cost is auto-captured from remaining energy**.
End-turn is a player decision too (an action, `EndPlayerTurnAction`). `CardSelectCmd.UndoEndTurnIfNecessary` (`CSC:1025`) un-readies a player who already pressed End Turn whenever a choice opens on the player side (also in single-player).

### 5.4 What can happen while a choice is pending (single-player)
* Card-play/potion choice: the whole engine is frozen except the pending action; no hooks fire; state is exactly as at the `await`.
* **Turn-start choice** (e.g. a drawn card autoplays and asks): `SetupPlayerTurn` is parked; the turn loop **continues** to `AfterSideTurnStart` and orb start hooks (`CM:778-796`) before awaiting the parked setup in `RunAutoPrePlayPhase` (`CM:861`). `UNVERIFIED`: whether any Necrobinder content can trigger this ordering.
* End-turn hooks (`BeforeSideTurnEnd` etc.): later listeners start before an earlier choosing listener finishes (§2.4); everything is joined before the phase proceeds.

## 6. RNG streams used in combat (names only; algorithm = separate task)
Run-scoped, created lazily as `new Rng(Seed, snake_case(name))` (`Runs/RunRngSet.cs:139-143`), persisted with the run (so counters carry across combats; `RunRngSet.cs:163`). Accessed via `RunState.Rng.X`.
| Stream | Advances at |
|---|---|
| `Shuffle` | initial deck shuffle (`CardPile.cs:71`, `Player.cs:814`); every reshuffle (`CPC:1088`); `CardPilePosition.Random` insertion (`CPC:509`); Stampede (`StampedePower.cs`) |
| `CombatTargets` | random-target attacks per hit (`ATK:617`); autoplay random target (`CCM:77,90`); Haunt/Cacophony/SerpentForm/Juggernaut/Countdown powers; LightningOrb |
| `MonsterAi` | `Monster.RollMove` (initial roll at combat start + each player-turn start, `MonsterModel.cs:418`); Flutter/Fabricator |
| `CombatCardGeneration` | generated-card selection (Calamity, CallOfTheVoid, HelloWorld, CreativeAi, many cards) |
| `CombatCardSelection` | random pick among existing cards (draw-pile `Random` position `CPC:1163`, Entropy, Improvement, Aggression, TrueGrit…) |
| `CombatEnergyCosts` | Confused/Slither random cost `NextInt(4)` |
| `CombatPotionGeneration` | Alchemize etc. |
| `CombatOrbs` (prop `CombatOrbGeneration`) | orb generation (unused by Necrobinder) |
| `Niche` | unique monster HP at spawn (`CS:241`), misc |
| `UpFront`, `UnknownMapPoint`, `TreasureRoomRelics` | not combat |
`PlayerRngSet` (Rewards, Shops, Transformations): transformations use `Owner.PlayerRng.Transformations` (relic-driven, mostly out of combat). Per-monster `monster.Rng`
seeded from run seed + map coord + act + `CombatId` (`CS:245`; consumers `UNVERIFIED`). `Rng.Chaotic` (time-seeded) is **cosmetic only** (VFX, tween jitter, `CPC:1115`): never port.
Shuffle comes in two flavours: `UnstableShuffle` (initial, `CardPile.cs:71`) and `StableShuffle` (reshuffle, `CPC:1088`) — different algorithms, separate task must cover both.

## 7. Existing history / trace types (`Combat.History`, `Combat.History.Entries`)
* `CombatHistory` (`CombatHistory.cs:14`): append-only `List<CombatHistoryEntry>`, cleared at combat end (`CM:1316`) and on `Reset`; entries only added while `IsLiveCombat`. Each entry carries `Actor` creature, `RoundNumber`, `CurrentSide`, per-player `TurnNumber` (`CombatHistoryEntry.cs`).
  Doc: "logged immediately after the event occurs, but before its AfterX hook".
* Types (17): CardPlayStarted, CardPlayFinished, CardDrawn(fromHandDraw), CardDiscarded, CardExhausted, CardGenerated, CardAfflicted, DamageReceived(DamageResult, dealer, cardSource), BlockGained(int, props, cardPlay),
  CreatureAttacked(results), EnergySpent, StarsModified, PowerReceived(power, amount, applier), PotionUsed, MonsterPerformedMove, OrbChanneled, Summoned.
* **Gaps as a trace format**: no entries for pile moves other than draw/discard/exhaust (flush discards, shuffles, add-to-hand are absent), HP changes only via DamageReceived/heal missing, no RNG draws, no hook boundaries, no player decisions,
  no turn/phase markers, no energy gain, no block loss/clear, no deaths. It exists for game logic (e.g. "cards played this turn"), so **it is a source of semantic ground truth to reproduce, not a sufficient oracle trace**. A separate trace schema is needed.
  `HumanReadableString` format: `Rd {Round} ({Side} turn): {Description}.`.
* Also `CombatManager` events (`CombatSetUp/Began/Ended/Won`, `TurnStarted/Ended`, `PlayerEndedTurn`) and `StateTracker.CombatStateChanged` (UI refresh) exist.

## 8. Recommendation — how a deterministic Rust engine should model this
Chosen: **explicit op/frame stack** (heap continuation frames, LIFO), not recursion with a callback.
Rationale: C# semantics = "depth-first, all hooks of a command finish before its continuation", which is exactly LIFO frame execution. Only calls taking a `PlayerChoiceContext`
can suspend (§2.4/§5), and choice points are the only places an agent acts, so the *suspended continuation must be plain data*.

| Option | Pros | Cons |
|---|---|---|
| **A. Op stack** (`Vec<Frame>`: HookDispatch{hook, snapshot ids, cursor, pass}, CardPlay{stage, i}, Damage{target idx, stage, …}, …; main loop pops/steps; a choice returns `Pending(Decision)`) | State is cloneable/serialisable (MCTS, RL snapshot, replay, no `dyn`/`Rc`); suspension = returning from `step`; listener-snapshot + `Contains` semantics map 1:1 to a cursor over a `Vec<ModelId>`; deterministic by construction | Every multi-step command hand-translated into a stage machine (large code, easy to mis-order "code after await"); results of awaited commands need slots (`Draw` returns cards, `Damage` returns results); harder to debug |
| **B. Native recursion + chooser callback** (`fn play(&mut State, &mut dyn Chooser)`) | Smallest port, near line-for-line with the decomp, lowest ordering risk; sync `Modify*`/`Should*` are plain fns anyway | Cannot snapshot/clone mid-effect or hand control to an external agent without threads/coroutines; `dyn` callback violates the "plain data" rule; no undo/branching at a choice |
| **C. Replay**: recursion as in B, but a choice with no recorded answer aborts the action; resume = restore pre-action snapshot, rerun with the decision log | Keeps B's port simplicity *and* plain-data state (snapshot + `Vec<Decision>`); deterministic since RNG counters live in state | Re-executes the effect per choice (cost × decisions, small); requires effects to be pure functions of (state, decisions) and a snapshot per action; two-choice cards pay O(k²) |

Suggested shape: **A for the spine** (turn loop, `OnPlayWrapper`, `Damage`, hook dispatch, draw/shuffle) written once; **card/power/relic bodies as short stage lists or small enums-with-state** that push frames. Use **C as the differential
test** (cheap to write per card, cross-checks A). The exact hook snapshot/`Contains`/guard rules (§2.2) and the "result pile chosen before `OnPlay`" rule (§3.2) must be explicit in the spine, not left to card code.

## 9. UNVERIFIED / open questions
1. Per-hook guard classification (§2.2 rule of thumb) across all 149 `Hook` methods.
2. Fork-join hook interleaving when a listener raises a choice (§2.4, §5.4); which Necrobinder content does it.
3. Whether deck-copy cards (run-level listeners, §2.3) ever execute hook bodies; and `Hook.IterateHookListeners(null)` vs combat-version differences for hooks that take `IRunState` with null combat.
4. `EnergyCost.GetAmountToSpend/CapturedXValue/AfterCardPlayedCleanup`, `GetStarCostWithModifiers`, `ModifyEnergyCostInCombat` fold order (not read).
5. Power amount/`AmountOnTurnStart` semantics and per-power `AfterSideTurnEnd` details beyond Weak/Vuln/Frail/Intangible.
6. `Monster` per-instance `Rng` (`CS:245`) consumers; `EncounterModel._rng`.
7. Interaction of End Turn with a pending choice beyond `UndoEndTurnIfNecessary` (already-queued `EndPlayerTurnAction` ids vs `CancelPlayCardActions`).
8. `Task.Yield` at `CM:1719` and `WaitForUnpause`/`Pause` — assumed non-semantic.
9. Mid-combat additions: `CreatureCmd.Add`, spawned monsters acting rules (`SpawnedThisTurn`), `ShouldStopCombatFromEnding` users.
