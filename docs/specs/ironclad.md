# Spec: porting Ironclad's missing Act 1 content (S-IRON)

Research only, no engine code. Source: `cargo xtask coverage --character Ironclad --act overgrowth --allow-missing --verbose` = 82 cards, 26 powers, 7 relics,
3 potions (118). Per-entity data: `cargo xtask inventory --character Ironclad <id>... [--deps]`. Paths under `ref/decomp/MegaCrit.Sts2.Core.`:
`C/` = `Models.Cards/`, `P/` = `Models.Powers/`, `R/` = `Models.Relics/`, `Po/` = `Models.Potions/`. Engine paths under `crates/sts2-core/src/`.
Siblings: `regent.md` §6 (Ironclad/Silent hook survey), `hooks.md`. Every batch lists its powers with the cards that apply them; a `P` suffix = power.

## 1. Result in one paragraph
Almost everything is content. Of regent.md §6's four gaps, **two are already closed** (`CardTag.Strike` is `CardModel::is_strike`, `content/cards/mod.rs:751`, used by
`StrikeIronclad` etc.; Shiv and Minion tags are not needed by any Ironclad card) and **two remain**: the `DamageReceived` history entry and the Fatal check. A third
(exhaust-pile hook listening) was checked and is fine. C18 below is therefore small (~80 lines). Checked and **not** gaps: `ModifyBlockMultiplicative` (wired, `hooks.md:93`),
`AfterAutoPostPlayPhaseEntered`, `AfterCardDrawnEarly`, `AfterBlockGained/Cleared`, `ShouldClearBlock`, `ModifyMaxEnergy`, `TryModifyPowerAmountReceived`, `AfterPowerAmountChanged`,
`ModifyCardPlayCount` (all `wired`), `CalculatedDamage` (`damage.rs:41`), `get_distinct_for_combat` / `pools::get_for_combat` (Stoke, InfernalBlade), `transform_to` (PrimalForce),
`auto_play` / `auto_play_from_draw_pile` (Havoc, Cascade, Hellraiser, Stampede, HowlFromBeyond), `choose_cards`, `gain_max_hp`, `heal`, `set_this_turn_or_until_played(0)` (= `SetToFreeThisTurn`).

## 2. Engine gaps and the proposed core task C18
| # | Gap | Blocks | Fix |
|---|---|---|---|
| G1 | `DamageReceived` history entry is not recorded (`history.rs:13` lists it unrecorded). Decomp `Combat.History.Entries/DamageReceivedEntry.cs`: `Result`, `Dealer`, `CardSource`, `Receiver`, round and side. | `Spite` (any entry this turn with receiver = owner and `UnblockedDamage > 0`, `C/Spite.cs`), `TearAsunder` (all entries this combat, same filter, `1 + count` hits, `C/TearAsunder.cs`) | Push one entry per `DamageResult` in `damage.rs` (where `AfterDamageReceived` fires); add `HistoryKind::DamageReceived { result, dealer, card_source }` and a `this_turn` query like the `CardExhausted` one. Shared with Regent `BeatIntoShape` and Silent `GangUp`, `EmotionChip`. |
| G2 | Fatal helper. `PowerModel.ShouldOwnerDeathTriggerFatal` exists (`powers/mod.rs:378`, `Minion` overrides it) but nothing calls it. | `Feed` (`C/Feed.cs`: read `target.Powers.All(...)` **before** the attack, then `Results.Any(WasTargetKilled)`) | `Engine::target_allows_fatal(creature) -> bool` plus a `fatal_triggered(&results)` helper beside `attack` (`damage.rs:22` has `was_target_killed`). Same for Silent `TheHunt`, Colorless `HandOfGreed`. |
| G3 | Content-only extras that need a decision, not core: `Rampage`/`Thrash` mutate their own `Damage.BaseValue` (`ExtraDamageFromPlays`/`ExtraDamage` re-added in `AfterDowngraded`); `CardData` is an enum (`cards/mod.rs:393`), so add two variants as `Bolas` did. | Rampage, Thrash | content |
| G4 | `Thrash` calls `Hook.ModifyDamage(..., null target, ValueProp.Move, otherCard)` for a **different** card; `hooks.rs:1334 modify_damage` must accept `target: None` and a `card_source` that is not the card being played. Verify while porting; widen the signature only if it cannot. | Thrash | maybe 10 lines in C18 |
Proposal: **C18 = G1 + G2 (+ G4 if needed)**, one Opus task, tests: a hit then `Spite` count, `TearAsunder` after N hits, `Feed` kills a Minion-power enemy (no max HP) and a normal one (+max HP). Batch 4 needs C18; `Thrash` (batch 3) needs G4; the rest do not.

## 3. Batches (dependency order; 118 = 25 + 24 + 21 + 24 + 24)
### Batch 1: plain attacks and skills, no new powers (23 cards, 2 potions)
Anger, AshenStrike, BloodWall, Bloodletting, Bludgeon, BodySlam, Breakthrough, Conflagration, Hemokinesis, IronWave, PactsEnd, PerfectedStrike, PommelStrike, Pillage, ShrugItOff,
SwordBoomerang, TwinStrike, Whirlwind, Impervious, ForgottenRitual, Offering, NotYet, ExpectAFight; potions `BloodPotion` (Heal), `SoldiersStew` (`base_replay_count += 1` on every Strike-tag card in `AllCards`, `Po/SoldiersStew.cs`).
* Self-damage cards (`Bloodletting`, `Hemokinesis`, `Offering`, `Breakthrough`): see §4.
* `AshenStrike` and `PerfectedStrike` use `CalculatedDamageVar` with a pile-size / Strike-count multiplier (`C/AshenStrike.cs`: Exhaust pile count; `C/PerfectedStrike.cs`: `AllCards` with the Strike tag). `PerfectedStrike`/`PommelStrike`/`TwinStrike` need `is_strike` overrides.
* Loadout/situations: `NotYet` and `BloodPotion` need HP below max (loadout `hp`); `BodySlam` needs block first; `Anger` copy goes to the discard pile (`AddGeneratedCardToCombat`, no rng); `SwordBoomerang` targets `RandomEnemy` (`CombatTargets` stream), so use a 2-enemy fight (`slimes_normal`, `nibbits_normal`).
* Encounters: none special.

### Batch 2: Vulnerable/Weak debuffs, Strength powers (17 cards + 6 powers + 1 relic)
Break, Bully, Dismantle, MoltenFist, Thunderclap, Taunt, Tremble, Uppercut, Dominate, Vicious + `ViciousPower`, Colossus + `ColossusPower`, Inflame, FightMe, Mangle + `ManglePower`, SetupStrike + `SetupStrikePower`,
Rage + `RagePower`, DemonForm + `DemonFormPower`; relic `PaperPhrog` (Vulnerable multiplier, read in `VulnerablePower`; hooks list empty).
* Pattern for temporary Strength: `ManglePower`/`SetupStrikePower` follow `content/powers/temporary_strength.rs` (`P/ManglePower.cs` hooks `BeforeApplied`, `AfterPowerAmountChanged`, `AfterSideTurnEnd`; `EnfeeblingTouch` is the ported template, `semantics/cards.md:53`). `DemonFormPower`: `AfterSideTurnStart` +Strength (`P/DemonFormPower.cs`). `RagePower`: block per Attack played, removed at turn end. `ColossusPower`: `ModifyDamageMultiplicative` vs Vulnerable attackers, ticks at `AfterSideTurnEnd`. `ViciousPower`: draws when Vulnerable **actually lands** (`AfterPowerAmountChanged`, `amount > 0`, `P/ViciousPower.cs`).
* **Artifact enemy** needed to test the negative path: `ArtifactPower` returns 0 for debuffs (`semantics/monsters.md:41`); Act 1 holder: `cubex_construct_normal` (Vicious must not draw, `Mangle`'s Strength loss must be blocked, `Dominate`'s Strength gain still happens). `kin_follower` (The Kin boss) has `Minion`: not for this batch (Fatal, batch 4). Debuff cards otherwise work on any encounter: `fuzzy_wurm_crawler_weak`, `slimes_weak`.
* Loadout: Colossus/Mangle/SetupStrike need an enemy that attacks next turn (any). `PaperPhrog`: relic in the loadout.

### Batch 3: exhaust and hand manipulation (16 cards, 3 powers, 1 relic, 1 potion)
Armaments, BurningPact, TrueGrit, SecondWind, Brand, Cinder, FiendFire, Thrash, Stoke, EvilEye, DrumOfBattle, PrimalForce + `GiantRock` (token), FeelNoPain + `FeelNoPainPower`, DarkEmbrace + `DarkEmbracePower`, Corruption + `CorruptionPower`;
relic `CharonsAshes` (`AfterCardExhausted`); potion `Ashwater` (`FromHand` exhaust prompt).
* Prompts: `Armaments` = `CardSelectCmd.FromHandForUpgrade` (`CardCmd.Upgrade` on the pick; read `C/Armaments.cs` for the upgraded variant), `BurningPact`/`TrueGrit`/`Brand`/`Ashwater` = `FromHand` with the exhaust prompt; all map to `choose_cards`, only the prompt kind differs. `TrueGrit` (unupgraded) and `Cinder`/`Thrash` pick via `CombatCardSelection` rng (`.NextItem`), not a prompt; `Stoke` exhausts the whole hand then generates that many cards (`C/Stoke.cs`).
* `Corruption`: Skills cost 0 (`TryModifyEnergyCostInCombatLate`) and exhaust (`ModifyCardPlayResultLocation`), `P/CorruptionPower.cs`. Needs a Skill-heavy deck.
* Loadout: attacks in hand for `Cinder`/`Thrash`/`PrimalForce` (only `IsTransformable` Attacks); a hand with 0 other cards for `Stoke`/`BurningPact` empty-hand path; `DarkEmbrace`/`FeelNoPain` need several exhausts in a turn (use `Corruption` + Skills, or `Havoc` in batch 5). Draw-pile-empty case for `DarkEmbrace` (reshuffle consumes `Shuffle`). No special enemies.

### Batch 4: HP loss, block rules, damage reactions (needs C18) (11 cards, 8 powers, 5 relics)
Rupture + `RupturePower`, Spite, TearAsunder, Feed, FlameBarrier + `FlameBarrierPower`, Inferno + `InfernoPower`, Juggernaut + `JuggernautPower`, Barricade + `BarricadePower`, Unmovable + `UnmovablePower`,
CrimsonMantle + `CrimsonMantlePower`, StoneArmor (applies the ported `PlatingPower`); relics `DemonTongue`, `SelfFormingClay` + `SelfFormingClayPower`, `Brimstone`, `RedSkull`, `RuinedHelmet`.
* `Feed`/G2: the enemy must die to the hit. Use a low-HP enemy (`slimes_weak` small slimes) and an unupgraded starter attack; **negative case** a `Minion` holder (`kin_follower`, `kin` boss) must give no max HP. `Inferno`/`FlameBarrier`/`DemonTongue`/`RedSkull`: any attacking enemy; `RedSkull` (`AfterCurrentHpChanged`, `AfterRoomEntered`, `AfterCombatEnd`) and `DemonTongue` (`AfterDamageReceived`, heals) need HP loss: set loadout `hp`/`max_hp`; read their thresholds in `R/`.
* `Barricade` (`ShouldClearBlock` false) and `SelfFormingClay` (`AfterBlockCleared`) need 2+ turns; `Juggernaut` draws one `CombatTargets` per block gain (`P/JuggernautPower.cs`: skipped for amount <= 0), so use a 2-enemy fight. `Unmovable` is a `ModifyBlockMultiplicative` power (read `P/UnmovablePower.cs` for the once-per-turn rule).
* `Rupture`: Strength only when the HP loss happens on the owner's side's turn; self-damage by the card being played is tallied per card and applied after the card (`P/RupturePower.cs`, `playedCards` data). Needs per-power internal data (as `Rage`/`Hang`).

### Batch 5: auto-play, generation, energy (15 cards, 9 powers)
Havoc, Cascade (`ResolveEnergyXValue()` X cost), InfernalBlade, Headbutt (`FromCombatPile` on the discard pile; add to the draw pile top), Stomp, Rampage, HowlFromBeyond, BattleTrance + `NoDrawPower`, Hellraiser + `HellraiserPower`, Stampede + `StampedePower`,
Juggling + `JugglingPower`, Aggression + `AggressionPower`, OneTwoPunch + `OneTwoPunchPower`, Unrelenting + `FreeAttackPower`, Pyre + `PyrePower` (`ModifyMaxEnergy`), `TankPower` (multiplayer only: `GuardedPower`, port last or skip).
* `Havoc`/`Cascade` play the top card from the draw pile (`auto_play_from_draw_pile`; `Havoc` forces exhaust; unplayable/curse tops and an empty pile need a case). `InfernalBlade` picks a random Attack via `CombatCardGeneration` and sets it free this turn. `Stomp` mirrors `content/cards/flatten.rs:98 reduce_cost` (`AfterCardEnteredCombat` + `BeforeCardPlayed`, counts Attack plays this combat in history, `C/Stomp.cs`).
* `Rampage`/`Thrash`: G3/G4. `AfterDowngraded` is wired; downgrade is reachable by no Act 1 source in a normal fight, so cover it with a unit test, not the referee.
* `HowlFromBeyond` triggers from the Exhaust pile in `AfterAutoPostPlayPhaseEntered` (`C/HowlFromBeyond.cs`): exhaust it first (batch 3 `Corruption`/`Havoc`), then end the turn. `Stampede` picks a hand Attack with the `Shuffle` stream; `Hellraiser` auto-plays each drawn Strike-tag card (`AfterCardDrawnEarly`, `P/HellraiserPower.cs`, infinite-loop cap of 9 per turn only when every enemy HP is infinite). `Aggression` moves discard Attacks to hand and upgrades them (`UnstableShuffle` with `CombatCardSelection`) at `BeforeSideTurnStart`.
* Loadout: strike-heavy deck (`Hellraiser`), 2+ attacks in hand (`Stampede`), attacks in discard (`Aggression`, `Headbutt`), `OneTwoPunch` + an Attack.

## 4. Ironclad mechanics a porter must know
* **Self-damage** is `CreatureCmd.Damage(owner, HpLoss, Unblockable | Unpowered | Move, source: owner, card)`, e.g. `C/Bloodletting.cs`, `C/Brand.cs`. The engine's `damage` supports both props; Strength/Vulnerable do not apply (Unpowered), block does not absorb (Unblockable). It fires `AfterDamageReceived` and `AfterCurrentHpChanged` (Rupture, DemonTongue, RedSkull) and, after G1, a history entry.
* **Exhaust synergies**: `AfterCardExhausted` fans out to `FeelNoPain`, `DarkEmbrace`, `CharonsAshes`, `DrumOfBattle` (wired, `hooks.md:66`); `AshenStrike` scales by Exhaust pile size; `EvilEye` doubles its block if a card was exhausted this turn (`WasCardExhaustedThisTurn`, history `CardExhausted`); ethereal exhaustion passes `causedByEthereal`.
* **Strength scaling**: `StrengthPower.ModifyDamageAdditive` adds `Amount` only to powered attacks by the owner (`P/StrengthPower.cs:16-24`), once per hit, so multi-hit cards (`TwinStrike`, `Thrash`, `Spite`, `SwordBoomerang`, `TearAsunder`) scale per hit; `Juggernaut`'s damage is `Unpowered` (`P/JuggernautPower.cs`), so Strength does not apply to it. `CalculatedDamageVar` cards add Strength after the multiplier (`damage.rs:41`).
* **Order effects**: `Feed`'s fatal check reads powers before the attack (G2); `Rupture`'s deferral; `Corruption` + `DarkEmbrace` + `FeelNoPain` interact through result location, so test the three together.
* **Rng**: `Cinder`/`Thrash`/`TrueGrit` `CombatCardSelection`; `InfernalBlade` `CombatCardGeneration`; `Stampede` `Shuffle`; `Juggernaut`/`SwordBoomerang` `CombatTargets`; `Aggression` `CombatCardSelection` (shuffle then take).

## 5. Referee notes
Use `loadout` (`deck`, `relics`, `potions`, `hp`, `max_hp`; `referee/README.md:47`) with `--character Ironclad`. Deck order fixes the draw order, so put the cards under test first. Run each batch's cards in groups of 5–8 (`cargo xtask hits` lists entities no generated run touched, T9a).
