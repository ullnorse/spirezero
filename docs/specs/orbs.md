# Spec: Defect orbs (S-MECH)

Research only, no engine code. Input for C16 (orbs core) and the Defect fan-out. Paths are under
`ref/decomp/MegaCrit.Sts2.Core.` (`Models.Orbs/X.cs` is `Models.Orbs/X.cs`; `C` = `Combat/CombatManager.cs`,
`H` = `Hooks/Hook.cs`, `AM` = `Models/AbstractModel.cs`, `OM` = `Models/OrbModel.cs`, `OQ` = `Entities.Orbs/OrbQueue.cs`,
`OC` = `Commands/OrbCmd.cs`). Engine paths are under `crates/sts2-core/src/`. Sibling specs: `hooks.md`, `regent.md`.

## 1. Model
* An orb is an `OrbModel` (`OM:23`), a combat-only `AbstractModel` with `ShouldReceiveCombatHooks => true` (`OM:161`).
  Five exist (`OM:27-34`, in this order, used by `GetRandomOrb`): `LightningOrb, FrostOrb, DarkOrb, PlasmaOrb, GlassOrb`.
  `Models.Orbs.Mock/*` are test mocks (ignore).
* Per-instance state: `Owner` (set once by `Channel`, `OM:137-153`), `HasBeenRemovedFromState` (`OM:47`, set by `RemoveInternal`
  after an evoke that dequeued, `OC:150`). Dark keeps `_evokeVal`, Glass keeps `_passiveVal`; both start fresh per `ToMutable()` (`OM:215`).
* Every orb overrides `PassiveVal`, `EvokeVal`, `Passive(ctx, target)` and `Evoke(ctx)`; `Evoke` returns the creatures affected
  (feeds `AfterOrbEvoked`). `Passive` is the raw effect; `TriggerPassive` (`OM:246-262`) wraps it with the trigger-count hooks.
* **No shipped orb overrides an `AbstractModel` combat hook.** Only the two queue entry points (`BeforeTurnEndOrbTrigger`,
  `AfterTurnStartOrbTrigger`, `OM:233-241`) are overridden. So the orb's slot in the listener order (section 5) never decides an
  outcome; the engine needs no `Listener::Orb`, only the orb list on `Player`.

## 2. Slots (`OrbQueue`, `OQ`)
* `Capacity` starts at `Player.BaseOrbSlotCount` for each combat (`Entities.Players/PlayerCombatState.cs:137-139`); a fresh `OrbQueue`
  each combat, so slots and orbs never carry over. `BaseOrbSlotCount` is `CharacterModel.BaseOrbSlotCount` (default 0,
  `Models/CharacterModel.cs:101`; **Defect 3**, `Models.Characters/Defect.cs:63`) copied at player creation (`Entities.Players/Player.cs:266`)
  and never changed by gameplay.
* Hard cap `maxCapacity = 10` (`OQ:15`). `OrbCmd.AddSlots(p, n)`: no-op once combat is over/ending; `n = min(10 - Capacity, n)`
  (`OC:23-33`). `OrbCmd.RemoveSlots(p, n)`: `n = min(Capacity, n)`; `RemoveCapacity` drops the **last** orbs while `Count > Capacity`,
  with no evoke and no hooks (`OC:41-49`, `OQ:41-48`).
* `Orbs` is a list; index 0 is the oldest ("front", evoked first), `TryEnqueue` appends (`OQ:50-64`). It returns false (orb lost) when
  `Capacity == 0`; it throws if full (cannot happen: `Channel` evokes first).
* Death clears the queue and capacity (`Commands/CreatureCmd.cs:578`, `OQ:30-34`).

## 3. Commands (`OC`)
* **`Channel(ctx, orb, player)`** (`OC:68-92`), skipped when `IsOverOrEnding`:
  1. If `player.Character.BaseOrbSlotCount == 0 && Capacity == 0`, `AddSlots(1)` (a slotless character gets one slot on first channel).
  2. `orb.Owner = player`. 3. If `Orbs.Count >= Capacity`: `EvokeNext(dequeue: true)` (evokes the front orb, full evoke hooks).
  4. `TryEnqueue`; **only if it succeeded**: `History.OrbChanneled` (`OrbChanneledEntry`, read by `Voltaic`), then
  `Hook.AfterOrbChanneled(player, orb)` (`H:856`).
  With `Capacity == 0` and base slots > 0 (after `BulkUp`) nothing is evoked or channeled. `Channel<T>` uses `ModelDb.Orb<T>().ToMutable()`.
* **`EvokeNext(ctx, p, dequeue = true)`** evokes `Orbs.First()`; **`EvokeLast`** evokes `Orbs.Last()` (`OC:94-116`); no-op on an empty queue.
* **`Evoke`** (private, `OC:125-153`): return if over/ending or empty; if `dequeue`, remove the orb from the queue **first**;
  `orb.Evoke(ctx)` yields the targets; if the owner is still in combat, `Hook.AfterOrbEvoked(orb, targets)` (`H:870`), then
  `RemoveInternal()` if it was dequeued. `dequeue: false` leaves the orb in place: it fires the full evoke and the hook, and can be
  evoked again. This is the "evoke twice" idiom: `Dualcast` = `EvokeNext(false)` then `EvokeNext(true)`; `Shatter` = that pair
  `Count` times (count read once, after its damage); `MultiCast` (X, +1 upgraded) = `EvokeNext` X times with `dequeue` only on the last,
  so it evokes the **front** orb X times and removes it once; `Quadcast` = 4 evokes of the front orb, dequeue on the last
  (`Models.Cards/{Dualcast:26-28,Shatter:34-36,MultiCast:29,Quadcast:30}.cs`).
* **`Passive(ctx, orb, target, countAffectedByHooks = false)`** (`OC:155-170`): over/ending skip; false calls the raw `orb.Passive`
  (**bypasses** `ModifyOrbPassiveTriggerCount`: `Loop`, `Darkness`, `TeslaCoil`); true calls `TriggerPassive` (`EmotionChip`).
* `RunState.Rng` is the run's rng set; `OrbModel.GetRandomOrb(rng)` is `rng.NextItem(_validOrbs)` (`OM:169`), 5 items.

## 4. Orbs: numbers and effects
`Focus` enters only through `ModifyOrbValue` (section 6). All damage is `ValueProp.Unpowered` from the owner (no Strength/Vulnerable
applied by the power pipeline; block uses `Unpowered`, so no Dexterity/Frail). Decimal values. "HE" = `CombatState.HittableEnemies`.

| Orb | Passive (trigger) | Evoke | Source |
|---|---|---|---|
| Lightning | `PassiveVal = Mod(3)`; damage to 1 enemy. Target: the card's target when `Passive(.., target)` is given (`TeslaCoil`), else `NextItem(opponents where IsHittable)` on `Rng.CombatTargets`. No hittable enemy: nothing, **no rng draw**. A list of 1 still draws (`NextInt(0,1)`). End of turn | `EvokeVal = Mod(8)`; same random-target rule (`target = null`); returns the target | `LightningOrb.cs:23-25,32-64`; `Rng.cs:289-299` |
| Frost | `Mod(2)` block to owner. End of turn | `Mod(5)` block to owner | `FrostOrb.cs:21-23,30-69` |
| Dark | `PassiveVal = Mod(6)`; `_evokeVal += PassiveVal` (no damage). End of turn. Throws if given a target | `EvokeVal = _evokeVal`, **starts at flat 6 (not Focus-modified)** and only grows by the Focus-modified passive; damage to the HE with the **lowest `CurrentHp`** (`MinBy`, first on ties); none: returns empty | `DarkOrb.cs:17,23-56` |
| Plasma | `1` energy (`PlayerCmd.GainEnergy`), not Focus-modified. **Start of turn** (`AfterTurnStartOrbTrigger`) | `2` energy to the owner | `PlasmaOrb.cs:17-42` |
| Glass | `PassiveVal = Mod(_passiveVal)`, `_passiveVal` starts 4 and drops by 1 (floor 0) **per passive**, before the damage; damage to **all** HE in one `Damage` call (kills resolve after). Skipped when `PassiveVal <= 0`. End of turn | `EvokeVal = PassiveVal * 2` (uses the decayed raw value); all HE; skipped (empty) when `<= 0` | `GlassOrb.cs:15-53` |

* `Frost` and `Plasma` hit the owner only (the `HibernatePower` multi-player branch, `FrostOrb.cs:39-49,57-66`, is multiplayer: skip).
* `ActivatePassive`/`ActivateEvoke`/sfx/`NCombatRoom` lines are presentation; do not port.

## 5. Timing in the turn
All `Passive` triggers go through `OrbQueue.BeforeTurnEnd` / `AfterTurnStart`, which iterate a `ToList()` snapshot and return early when
the owner has left combat (`OQ:80-102`). The trigger itself is not guarded by `IsOverOrEnding`, so a Lightning passive that kills the
last enemy lets later orbs in the snapshot still run (their damage finds no hittable enemy).
* **Turn start** (player side only, `C:783-796`): `BeforeSideTurnStart` (`CrackedCore` channels 1 Lightning on turn 1, `CrackedCore.cs:29`),
  block clear, **energy reset -> `AfterEnergyReset(+Late)`** (`LightningRod`, `Spinner` channel here), `BeforeHandDraw`, draw,
  `AfterPlayerTurnStart` (`Loop`, `EmotionChip`), `AfterSideTurnStart` hook (`Coolant`, `BiasedCognition`, `InfusedCore` T1,
  `RunicCapacitor` T1, `SymbioticVirus` T1), **then `OrbQueue.AfterTurnStart`** (Plasma), then the auto-pre-play phase.
* **Turn end** (`C:1542-1590,1599-1603`): `AfterAutoPostPlayPhaseEntered`, `BeforeSideTurnEnd` (`Hailstorm`), `CheckWinCondition`,
  **`DoTurnEnd` opens with `OrbQueue.BeforeTurnEnd`** (Lightning, Frost, Dark, Glass passives, oldest first), then the Ethereal exhaust and
  turn-end-in-hand cards (a `IsCombatEnding` check sits between), `BeforeFlush`, hand flush, `AfterSideTurnEnd` (`ConsumingShadow`
  evokes, `TemporaryFocus` removes its Focus). So Focus lasts through the end-of-turn passives.
* Combat start: the orb queue exists empty with `Capacity = 3` before `BeforeSideTurnStart` of turn 1.
* Inside one trigger: `TriggerPassive` = `Hook.ModifyOrbPassiveTriggerCount(1)` -> `AfterModifyingOrbPassiveTriggerCount`
  to the changers -> `Passive` repeated `count` times with a (cosmetic) wait between (`OM:246-262`).

## 6. Hooks and their position in the listener order
Listener order (`Combat/CombatState.cs:411-470`, `docs/semantics/combat-core.md`): allies then enemies; per player creature: powers,
relics, potions, **orbs** (`CombatState.cs:449`), then cards by pile (Hand, Draw, Discard, Exhaust, Play; each card then its Affliction and
Enchantment). An orb listens while `!HasBeenRemovedFromState && Owner.IsActiveForHooks` (`CombatState.cs:588`).

| Hook (`AM` line) | Dispatcher (`H`) | Guard | Listeners that matter | Engine status |
|---|---|---|---|---|
| `ModifyOrbValue(orb, value)` `AM:1829` | `H:1882-1890`, a fold over all listeners | combat (silent once over/ending) | `FocusPower` (`FocusPower.cs:14`: `max(value + Amount, 0)` if `Owner.Player == orb.Owner`), `InfusedCore` (+1 to Lightning only, `InfusedCore.cs:46`). Fold order: powers, then relics | missing |
| `ModifyOrbPassiveTriggerCounts(orb, count)` `AM:1524` | `H:1863-1876`; collects changers | combat | `GoldPlatedCables`: +1 when `orb` is `Orbs[0]` of the owner (`GoldPlatedCables.cs:10`) | missing |
| `AfterModifyingOrbPassiveTriggerCount(orb)` `AM:870` | `H:796-805` | combat | `GoldPlatedCables` (flash only) | missing |
| `AfterOrbChanneled(ctx, player, orb)` `AM:977` | `H:856-865` | combat | `Metronome` counts per owner, deals 30 to HE at the 7th channel (`Metronome.cs:89`) | missing |
| `AfterOrbEvoked(ctx, orb, targets)` `AM:988` | `H:870-877` | combat | `ThunderPower`: owner's Lightning evoke -> `Amount` Unpowered damage to each living target (`ThunderPower.cs:26`) | missing |

`AfterOrbChanneled` pushes each listener on the choice context (`H:860`), `AfterOrbEvoked` does not. `ModifyOrbValue` has no companion
`AfterModifying*` hook.

## 7. Orb-related cards, powers, relics, potions (Defect)
Card IDs (all `Models.Cards/`): `Zap`/`BallLightning`/`Tempest`/`Voltaic`/`Rainbow`/`TeslaCoil`/`Storm`/`LightningRod`/`Thunder` Lightning;
`Coolheaded`/`ColdSnap`/`Chill`/`Glacier`/`IceLance`/`Hailstorm`/`Coolant` Frost; `Darkness`/`ShadowShield`/`Null`/`ConsumingShadow` Dark;
`Fusion`/`MeteorStrike` Plasma; `Glasswork`/`Refract`/`Spinner` Glass; `Chaos`/`TrashToTreasure` random; `Capacitor`/`Modded`/`BulkUp`
slots; `Defragment`/`BiasedCognition`/`FocusedStrike`/`Hotfix`/`Synchronize`/`Hyperbeam` Focus; `Loop`/`Dualcast`/`Quadcast`/`MultiCast`/`Shatter` evoke or poke; `Barrage`/`CompileDriver` count orbs.
`Hibernate` and `Ignition` are `MultiplayerOnly`. Starter: 4 `StrikeDefect`, 4 `DefendDefect`, `Zap`, `Dualcast`; 75 HP, 99 gold, relic
`CrackedCore` (`Defect.cs:27-53`).
* Channel counts: `Zap` 1 Lightning; `BallLightning` 1; `Tempest` X (+1 upgraded); `Voltaic` = number of Lightning channels this combat by the
  owner (`History.Entries.OfType<OrbChanneledEntry>`, `Voltaic.cs:30`); `Rainbow` L+F+D; `Chill` 1 Frost per **hittable enemy**
  (`Chill.cs:29-34`); `Glacier` 2 Frost; `IceLance` `Repeat` 3 Frost; `MeteorStrike` 3 Plasma; `Refract` `Repeat` Glass; `Chaos` `Repeat`
  random orbs from `CombatOrbs`; `ConsumingShadow` `Repeat` 2 Dark.
* Counting cards: `Barrage` hits = `Orbs.Count` (`Barrage.cs:25`); `CompileDriver` cards = number of **distinct orb ids**; `Synchronize` Focus
  = distinct ids x `CalculationExtra` (`.Count()` of `GroupBy(Id)`). Calculated vars read the live queue at `Calculate` time.
* Passive pokes: `Darkness` channels Dark then `OrbCmd.Passive` on every Dark orb (1x, 2x upgraded; raw, bypasses trigger-count hooks);
  `TeslaCoil` raw Passive on each Lightning orb with the card target (1x, 2x upgraded) after its damage.
* Powers: `Loop` (`AfterPlayerTurnStart`: raw Passive of `Orbs[0]`, `Amount` times, `Loop.cs:15`); `Storm` (per Power card played by owner,
  snapshot of `Amount` at `BeforeCardPlayed`, channels that many Lightning in `AfterCardPlayed`, `Storm.cs:39,53`); `LightningRod`
  (`AfterEnergyReset`, 1 Lightning then `Decrement`, `:25`); `Spinner` (`AfterEnergyReset`, `Amount` Glass, `:24`); `Coolant`
  (`AfterSideTurnStart`: block = distinct orb ids x `Amount`, Unpowered, `:21`); `Hailstorm` (`BeforeSideTurnEnd`: if >= 1 Frost orb, `Amount`
  Unpowered damage to HE, `:30`); `ConsumingShadow` (`AfterSideTurnEnd`: `EvokeLast` `Amount` times if the queue is non-empty, `:18`);
  `BiasedCognition` (`AfterSideTurnStart` -`Amount` Focus, `:21`); `TrashToTreasure` (`AfterCardGeneratedForCombat`: for a Status card created by
  the owner, `Amount` random orbs from `CombatOrbs`, `:16`).
* **Temporary Focus** (`TemporaryFocusPower`, subclasses `FocusedStrikePower`, `HotfixPower`, `SynchronizePower`, and negative
  `HyperbeamFocusDownPower`): `BeforeApplied` applies `Sign * amount` real `FocusPower` silently; `AfterPowerAmountChanged` re-applies
  the delta; at the owner's `AfterSideTurnEnd` it removes itself and applies `-Sign * Amount` Focus (`TemporaryFocusPower.cs:130-150`).
* Relics: `CrackedCore` 1 Lightning on `BeforeSideTurnStart` turn 1; `InfusedCore` 3 Lightning on `AfterSideTurnStart` turn 1 and +1 Lightning
  value; `SymbioticVirus` 1 Dark on turn 1 (`AfterSideTurnStart`); `RunicCapacitor` +3 slots on turn 1; `DataDisk` +1 Focus in a `CombatRoom`;
  `EmotionChip` at `AfterPlayerTurnStart`, if the owner lost HP last turn (`DamageReceivedEntry`), `Passive(countAffectedByHooks: true)` on every orb;
  `GoldPlatedCables`; `Metronome`. Potions: `FocusPotion` +2 Focus, `PotionOfCapacity` +2 slots, `EssenceOfDarkness` one Dark per slot
  (`Capacity`, `EssenceOfDarkness.cs:28`).

## 8. RNG streams
* `CombatOrbs` (`snake_case` `combat_orbs`, `Runs/RunRngSet.cs:88`): one `NextItem` over the 5-orb array per random orb (`Chaos`,
  `TrashToTreasure`). Already declared in `rng/sets.rs:20,55`.
* `CombatTargets` (`RunRngSet.cs:72`): one `NextItem` per random Lightning passive/evoke (target-less), over the hittable opponents in
  `GetOpponentsOf` order; draws even for 1 enemy; no draw if none.
* No other orb code draws rng. Run-scoped streams persist across combats (`docs/semantics/combat-core.md`).

## 9. Engine gaps (`crates/sts2-core`)
Status (C16): O1-O7 and O10 are done (`combat/commands/orb.rs`, `content/powers/focus.rs`, `combat/history.rs`
`OrbChanneled`, `sts2-diff` `orbs_json`). The `TemporaryFocus` subclasses (O3), O8 and O9 remain; ported cards so far:
`Zap`, `Dualcast`, `Coolheaded`, `Darkness`, `Fusion`, `Glasswork`, `Chaos`, `Defragment`, `Capacitor`, `BulkUp`, plus `CrackedCore`.

| # | Gap | Where it bites | Notes / size |
|---|---|---|---|
| O1 | No orb state: `combat/state.rs` `Player` has no orb queue, capacity or `OrbKind`; `Character`/`CombatConfig` has no `BaseOrbSlotCount`. | everything | `Vec<Orb{kind,data}>` + `capacity`; Dark `evoke_val`, Glass `passive_val` as data; `BaseOrbSlotCount` from the character (C15) |
| O2 | `OrbCmd` family absent: `channel`, `evoke_next`, `evoke_last`, `evoke(dequeue)`, `passive(count_hooks)`, `add_slots`, `remove_slots`. | all orb cards | `combat/commands/orb.rs`; needs `Unpowered` damage to a target list, `GainBlock`, `GainEnergy` (exist) |
| O3 | `Focus` power and the `ModifyOrbValue` fold (+ `InfusedCore`); `TemporaryFocus` subclasses (`temporary_strength.rs` is the template). | Focus cards/potion/relic | new dispatcher in `hooks.rs`; `AllowNegative` true |
| O4 | Hooks `ModifyOrbPassiveTriggerCount`, `AfterModifyingOrbPassiveTriggerCount`, `AfterOrbChanneled`, `AfterOrbEvoked` not in `hooks.md`/`hooks.rs`. | `GoldPlatedCables`, `Metronome`, `Thunder` | add rows to `hooks.md`; Combat guard |
| O5 | Turn loop has no orb triggers: `turn.rs` needs `OrbQueue.AfterTurnStart` after `after_side_turn_start`, and `BeforeTurnEnd` at the top of `do_turn_end` (before the Ethereal exhaust), each over a snapshot with the owner-in-combat check. | all passives | two call sites |
| O6 | `History`: no `OrbChanneled` entry (`combat/history.rs:14` lists it unrecorded); `DamageReceived` also missing (`EmotionChip`). | `Voltaic`, `EmotionChip` | add `HistoryKind::OrbChanneled{kind}` + `DamageReceived` |
| O7 | `Rng`: `CombatOrbs` declared; `GetRandomOrb` helper (order Lightning, Frost, Dark, Plasma, Glass) missing. | `Chaos`, `TrashToTreasure` | `rng` already has `CombatTargets` |
| O8 | Calculated vars over the queue (`Barrage`, `CompileDriver`, `Synchronize`: count, distinct ids); `CalculatedVar` exists for damage only. | 3 cards | extend `CalculatedDamage` multiplier source |
| O9 | `CardKind`s: none of the Defect pool (91 cards), 8 relics, `FocusPotion`/`PotionOfCapacity`/`EssenceOfDarkness` and their powers exist. | fan-out | content batches after C16 |
| O10 | Tracer/compare must dump orbs (kind, passive/evoke values, capacity) per `docs/history/characters-plan.md` T10. | referee diff | note: `GlassOrb._passiveVal`/`DarkOrb._evokeVal` are hidden state; dump the computed `PassiveVal/EvokeVal` |
| O11 | `Hibernate`, `Ignition` and the Frost `HibernatePower` branch are multiplayer-only. | none | not a gap; do not port |
