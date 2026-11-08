# Content API: adding cards, powers, relics, potions, monsters, encounters, events, ancients

All content lives in `crates/sts2-core/src/content/<kind>/`, one file per entity, named after the
C# class in snake case (`ShrinkPower` -> `powers/shrink.rs`). Port the body from the decomp line by
line and cite it (`Models.Powers/ShrinkPower.cs`) in the file's doc comment.

## The pattern
Each entity is a struct implementing two traits:
- the kind trait (static data, entry points): `CardModel`, `PowerModel`, `RelicModel`, `PotionModel`,
  `MonsterModel`, `EncounterModel`, `AfflictionModel`, `EnchantmentModel` (in the kind's `mod.rs`). Every kind has a `name()`, the English
  `Title` (monsters: `<ENTRY>.name`) from the game's `localization_eng/<kind>.json`, in the entity's own file;
- `Hooks` (`combat/hooks.rs`), one method per hook pass (`after_side_turn_end`, `..._late`, ...). Every
  method defaults to "do nothing", so override exactly the hooks the C# class overrides, with the
  same body. `Me` is the listener's id: `PowerRef`, `CardId` (cards, and the affliction or enchantment
  on that card), `RelicId`, `PotionId` or `CreatureId` (monsters).
  Async hooks get `&mut Engine` and return `Flow`; value hooks (`modify_*`, `should_*`, `try_modify_*`)
  get `&CombatState`. Hook arguments come as small structs where the C# signature is long:
  `DamageQuery`, `HpLossQuery`, `BlockQuery` (`card` = `cardSource`, `play` = `cardPlay`; `gain_block` takes the
  `Option<&CardPlay>` that caused it), `DamageEvent` (target, `DamageResult`, props, dealer,
  card), `PowerQuery` (kind, target, applier, card = `cardSource`; a new power is not on its target yet). `TryModify*`
  returns `Some(new value)` when it modifies. Every hook any Act 1 entity uses is in the table below.
  Put `#[sts2_macros::hooks]` on the `impl Hooks for X` block (even an empty one): it records which
  hooks the block overrides, so dispatch skips the model for the others. Without it the model is
  called for every hook (still correct, only slower).

## Registration
Adding the file is all: there is no list to edit. `crates/sts2-core/build.rs` scans each
`content/<kind>/` directory and generates the kind's registry (included at the top of its `mod.rs`):
the module declarations, the enum with one variant per file, `ALL`, `id()`, `from_id()` and `model()`.
- The file `<snake_case>.rs` must declare `pub struct <CamelCase>` (`right_hand_hand.rs` ->
  `pub struct RightHandHand`), which names the variant (`CardKind::RightHandHand`). Its `///` doc comment
  becomes the variant's. Otherwise the build fails and says so.
- `id()` is `ModelId.Entry`, which is also the referee's id (`crates/sts2-diff/src/ids.rs` forwards to it):
  the struct name in screaming snake case, plus `_POWER` for powers (`LeafSlimeM` -> `LEAF_SLIME_M`,
  `Shrink` -> `SHRINK_POWER`). If the C# entry differs, declare it in the file:
  `pub(crate) const ID: &str = "...";`.
- Monsters: the variant holds the struct (`MonsterKind::Nibbit(Nibbit { .. })`, `MonsterKind::Osty(Osty)`),
  which is re-exported from `monsters`; monsters have no `ALL`.
- Exceptions are marked in the source. A test-only entity has `#![cfg(test)]` after its `//!` doc: its
  variant only exists under `cfg(test)` and it stays out of `ALL` and `from_id` (`cards/test_choice.rs`).
  A shared module that is not an entity (`cards/pools.rs`, `powers/temporary.rs`, `monsters/machine.rs`,
  `relics/egg_relic_helper.rs`) is declared by hand in `mod.rs`, which tells the build script to skip it.
- Nothing else in the core changes: dispatch goes through `model()`. Per-instance data still needs a
  variant in the kind's `*Data` enum (`CardData`, `PowerData`, `RelicData`, `EventData`).
  A new `PowerData` (or other traced `*Data`) variant also needs an arm in `crates/sts2-diff/src/fields.rs` with exactly the field names the referee tracer emits (XT3: keys on only one side diverge; the match is exhaustive, so the build fails without it).
- `ALL` is in file-name order; never depend on it for game behaviour (the C# pools have their own order).

## Display tables (C27)

What the game declares only for display lives in generated tables, `content/tables/*.rs`, made by `python3 tools/gen_tables.py` from the decomp: each monster `MoveState`'s intents (`CombatState::intents`), and the relic, potion, event and power `CanonicalVars` (`RelicKind::vars`, `PotionKind::vars`, `EventRoom::vars`, `PowerKind::vars`). They cover every model of the game, ported or not, so a porter adds nothing; rerun the script only if the decomp changes. A model whose text reads a var it keeps in its own fields (`WingedBoots.Rooms`, `SlowPower.DisplayAmount`, an event's rolled gold) maps the field in its kind's `display.rs`; relic counters (`ShowCounter`/`DisplayAmount`) are ported in `content/relics/display.rs`. A card var's preview class and `Props` (`DamageVar(2m, Unpowered | Move)`) are declared on its `CardVar` (`.props`, `.block()`, `.from_osty()`); `CombatState::card_preview` is `UpdateDynamicVarPreview`.

## Kinds

| Kind | Copy | Enum / registry | Notes |
|---|---|---|---|
| Card | `cards/slimed.rs` (status), `cards/strike_necrobinder.rs` (attack), `cards/sow.rs` (all enemies), `cards/eradicate.rs` (X cost), `cards/wisp.rs` + `cards/eidolon.rs` (upgrade changes keyword / cost) | `CardKind` / `CardKind::model` | see Cards below |
| Power | `powers/shrink.rs` (hooks), `powers/strength.rs`, `powers/frail.rs` (duration), `powers/slippery.rs` (HP loss), `powers/vigor.rs` (instance data) | `PowerKind` / `PowerKind::model` | `power_type`, `instance_type` (`PowerInstanceType::{None, Instanced, InstancedPerApplier}`; copy `powers/the_bomb.rs`), `allows_negative`, `is_visible`, `should_owner_death_trigger_fatal(s, me)` (reads the state: `powers/reattach.rs`), `owner_is_secondary_enemy`; lifecycle `before_applied`/`after_applied`/`after_removed` (gets the removed instance, so it can read its `amount` and `data`: `powers/sword_sage.rs`); read the amount with `s.power(me.owner, me.id)`; duration debuffs tick with `tick_down_duration`; `InitInternalData` is `init_internal_data` returning a `PowerData` variant, read with `s.power(..).data`, written with `s.power_mut(..)` |
| Relic | `relics/bound_phylactery.rs`, `relics/nunchaku.rs` (instance data), `relics/bread.rs`; run hooks: `relics/prayer_wheel.rs` (rewards), `relics/dragon_fruit.rs` (gold), `relics/pomander.rs` (`AfterObtained` with a prompt), `relics/silver_crucible.rs` (data, card rewards, treasure) | `RelicKind` / `RelicKind::model` | see Relics below |
| Monster | `monsters/twig_slime_m.rs` (random branch), `monsters/flyconid.rs` (cooldowns), `monsters/nibbit.rs` (conditional), `monsters/inklet.rs` (`after_added_to_room`), `monsters/fogmog.rs` (weights, summon) | `MonsterKind` / `MonsterKind::model` | see below |
| Encounter | `encounters/slimes_weak.rs`, `encounters/fogmog_normal.rs` (slots) | `Encounter` / `Encounter::model` | `generate_monsters(&mut Rng)` gets the encounter's own `Rng`; return monsters in spawn order; `slots()` is `Slots`, `spawn_slots()` the slot of each spawned monster; `should_give_rewards()` false (`ShouldGiveRewards`) skips the won room's rewards screen (`encounters/battleworn_dummy_event_v1_encounter.rs`) |
| Affliction | `afflictions/entangled.rs` | `AfflictionKind` / `AfflictionKind::model` | `can_afflict_card_type`, `can_afflict_unplayable_cards`, `is_stackable`; listens after its card |
| Enchantment | `enchantments/vigorous.rs` (damage), `enchantments/slither.rs` (hook, `can_enchant_extra`), `enchantments/swift.rs` (`on_play`), `enchantments/royally_approved.rs` (`on_enchant`), `enchantments/nimble.rs` (block) | `EnchantmentKind` / `EnchantmentKind::model` | `enchant_damage_*` / `enchant_block_*` (read the instance's `amount`/`status`/`extra_damage`), `on_play` (after the card's own `OnPlay`), `on_enchant` (a `&mut Card`; `enchantments::modify_card`), `can_enchant_card_type`; listens after its card's affliction |
| Ancient | `ancients/test_ancient.rs`, `ancients/neow.rs` | `AncientKind` / `AncientKind::model` | see Ancients below |
| Potion | `potions/fire_potion.rs` (enemy), `potions/block_potion.rs` (player) | `PotionKind` / `PotionKind::model` | see below |

## Cards
- Static data: `cost` (`CanonicalEnergyCost`), `has_energy_cost_x`, `card_type`, `rarity` (`CardRarity`,
  from the constructor's `base(..)` call; `sts2-diff` reads it), `target`, `keywords`
  (`CanonicalKeywords`, e.g. `Keywords::NONE.with(CardKeyword::Retain)`), `max_upgrade_level`.
- `DynamicVars` (`content/cards/vars.rs`): declare every `CanonicalVars` entry once, in order, in
  `vars()`, with its `OnUpgrade` step: `CardVar::new(Var::Damage, 6).upgrade(3)` for
  `new DamageVar(6m, ..)` + `DynamicVars.Damage.UpgradeValueBy(3m)`. Keys: `Var::Damage`, `Block`,
  `Cards`, `Energy`, `Repeat`, `Stars`, `Summon`, `OstyDamage`, `HpLoss`, `Forge`, `Gold`, `MaxHp`, `Heal`,
  `CalculationBase`, `CalculationExtra`, `ExtraDamage`, `CalculatedDamage`, `CalculatedBlock`,
  `Power(PowerKind::X)` (`PowerVar<XPower>`), `Named("Increase")` (an explicit name);
  `CardVar::decimal` for a fractional base. Body (`OnPlay`, hooks) reads them, never literals:
  `eng.s.card(play.card).var(Var::Damage)` (`BaseValue`, a `Decimal`), `.int_var(..)` (`IntValue`),
  `.count_var(..)` (a non-negative `IntValue` as `u32`, for hit and card counts), `.try_var(..)`
  (`ContainsKey`). A calculated var (`CalculatedVar.WithMultiplier`) is declared with its multiplier
  fn, `CardVar::calculated(Var::CalculatedDamage, strike_cards)` beside its `CalculationBase` and
  extra var (`ExtraDamage` for `CalculatedDamage`, else `CalculationExtra`), and calculated with
  `eng.s.calculate(card, var, target)` (an attack: `AttackDamage::Calculated(play.card)`). A var the
  class mutates (`Rampage`, `Thrash`, `TheScythe`, `SovereignBlade`) keeps its change in `CardData`
  and overrides `instance_var`. `is_upgraded()` is only for logic the C# gates on `IsUpgraded`.
  Outside the engine: `CardKind::vars`, `CardKind::base_var(var, level)`, `Card::vars()`; a deck card's are
  `RunCard::vars()` (built through `Card::from_deck`, so `instance_var` and saved counters apply; a calculated var's
  `Calculate(null)` is its `CalculationBase` out of combat).
- `on_play(eng, &CardPlay)`.
- Per-instance type (`CardModel.Type` is virtual): override `instance_card_type(&Card)` /
  `instance_gains_block(&Card)` (and `target_in`) to read the card's own `CardData`; copy `cards/mad_science.rs`.
  Code that asks a card instance's type uses `Card::card_type()` / `Card::gains_block()`
  (`s.card(id).card_type()`), never `kind.card_type()`, which is the canonical card's.
- `result_location_for_card_play(base)` (`GetResultLocationForCardPlay`): gets the base location
  (None for powers, Exhaust, else Discard at the bottom; the engine consumes `ExhaustOnNextPlay`) and returns
  where the played card goes, before `ModifyCardPlayResultLocation`. `ResultPile::{None, Exhaust, Discard,
  Draw, Hand}` (a full hand discards); copy `cards/shining_strike.rs` (draw top), `cards/particle_wall.rs` (hand).
  The instance-state part of `OnUpgrade` goes in `on_upgrade(&mut Card)`: `card.keywords.insert/remove`
  (`AddKeyword`/`RemoveKeyword`), `card.energy_cost.upgrade_by(n)` (`EnergyCost.UpgradeBy`).
- Instance state lives on `Card` (`eng.s.card(id)`): `keywords`/`has(kw)` (local keywords),
  `energy_cost` (`CardEnergyCost`: `set_this_turn`, `set_this_combat`, `set_until_played`,
  `set_this_turn_or_until_played` (0 is `SetToFreeThisTurn`), `add_this_turn`, `add_this_combat`, ...),
  `exhaust_on_next_play`, `single_turn_retain`, `single_turn_sly`, `affliction`, `enchantment`,
  `clone_of`/`is_clone()` (`CloneOf`/`IsClone`, set by `create_clone`), and `data`
  (a `CardData` variant for the card class's own fields, e.g. `TheScythe`). The engine handles Exhaust,
  Ethereal, Retain, Innate, Unplayable and Sly from these, and clears the turn-scoped state in `EndOfTurnCleanup`.
- Replays: `Card::base_replay_count` (`BaseReplayCount`, `Transfigure` raises it) feeds the play count;
  `CardPlay::energy_value` is `Resources.EnergyValue` (energy spent, or the cost of an auto-play).
  `CardPlay::id` is the play's identity (the game compares `CardPlay`s by reference; `UnmovablePower`).
- X cost: `eng.s.resolve_energy_x_value(play.card)` (`ResolveEnergyXValue`, runs `ModifyXValue`).
- Targets: `TargetType::AllEnemies` cards get no target; attack with `AttackTargets::AllOpponents`
  (`TargetingAllOpponents`). `TargetType::RandomEnemy` cards get no target either; attack with
  `AttackTargets::RandomOpponents` (`TargetingRandomOpponents`: one random living opponent per hit,
  `cards/sweeping_gaze.rs`). Hit counts: `Attack::new(..).with_hits(n)` (`WithHitCount`).
- The room: `s.room_type` (`CurrentRoom.RoomType`: `Monster`, `Elite` or `Boss`, from the encounter;
  `relics/sling_of_courage.rs`).
- Deck versions: a combat card cloned from the deck has `deck_version` (`DeckVersion`), an index into
  `s.deck`; a card that changes its deck version (`TheScythe.BuffFromPlay`) writes `s.deck[i].counter`,
  which the run reads back into `RunCard::counter` after the fight (`DeckCard::counter` seeds the
  clone's `CardData` via `cards::data_from_deck`).
- Other hooks: `is_playable` (`IsPlayable`), `has_turn_end_in_hand_effect` + `on_turn_end_in_hand`,
  sync `after_created` and `after_downgraded`.
- Keywords: a card's `Keywords` is its local set plus the global ones listeners add in
  `try_modify_keywords_in_combat(s, me, card, &mut Keywords) -> bool` (`TryModifyKeywordsInCombat`;
  `HexPower` adds Ethereal to Hexed cards; `powers/test_hex.rs` is the test-only example). Mutate the set in
  place, never `s.card_mut(..).keywords` (global keywords are never stored). Read keywords with
  `s.has_keyword(card, kw)` / `s.keywords(card)` (`eng.hooked().has_keyword(..)` in the engine), and
  `s.should_retain_this_turn(card)` / `s.is_sly_this_turn(card)`; `Card::has_local` / `local_keywords` are
  `LocalKeywords` only (canonical, `AddKeyword`, `RemoveKeyword`).
- `CombatHistory` is indexed, not scanned: `s.history_this_turn(HistoryTag::X)` (`HappenedThisTurn`, this
  turn's entries of one kind), `s.history_count(tag)` and `s.ethereal_card_plays_finished()` (combat-wide
  counts). Earlier turns survive only as counts; a new query over them needs a new index in
  `combat/history.rs` (a core change). Entries are `HistoryEntry { actor, round, side, turn_number, kind }` with
  `HistoryKind::{CardPlayStarted, CardPlayFinished { was_ethereal, card_type, .. }, CardDrawn { from_hand_draw, .. },
  BlockGained { amount, props, play },
  CardExhausted, CardGenerated, EnergySpent, CreatureAttacked, PowerReceived}`.

### Card mechanics survey (86 single-player Necrobinder cards, C5)
| Mechanic | Cards | Engine |
|---|---|---|
| Exhaust | Afterlife, Cleanse, Dirge, Dredge, Eidolon, Graveblast, Putrefy, Reanimate, SharedFate, TheScythe, Transfigure, Wisp | done |
| Ethereal | BansheesCry, CallOfTheVoid, Defile, Defy, Demesne, EnfeeblingTouch, Fear, Lethality, Pagestorm, Parse, Seance, SpiritOfAsh, Veilpiercer | done |
| Retain (keyword, single-turn) | Eradicate, Reap, Sacrifice, Sow, Spur | done |
| Innate | CallOfTheVoid+ | done |
| Unplayable | read by Eidolon (no Necrobinder card has it) | done |
| X cost, `ModifyXValue` | Eradicate, Dirge; Transfigure reads `CostsX` | done |
| Upgrade adds / removes a keyword | CallOfTheVoid, Dredge, Misery, ReaperForm, TimesUp, Wisp / Graveblast, Transfigure | done (`on_upgrade`) |
| Upgrade changes cost | BansheesCry, Demesne, Eidolon, ForbiddenGrimoire, Pagestorm, Protector, Sacrifice, Seance, SentryMode | done (`upgrade_by`) |
| Cost modifiers (turn, combat, until played, free) | BansheesCry, Melancholy, Transfigure (this combat), Flatten (this turn) | done |
| `CardCmd.ApplyKeyword` in combat | SculptingStrike, Snap | done (`Engine::apply_keyword`) |
| `CardCmd.Upgrade` in combat | DrainPower, Dirge, Reave | done (`Engine::upgrade_card`) |
| AllEnemies | BansheesCry, BoneShards, Deathbringer, EndOfDays, HighFive, NegativePulse, Sow | done |
| Hit count | Eradicate, PullFromBelow, Rattle | done |
| `CardCmd.AutoPlay` (random AnyEnemy target) | Eidolon | done (`Engine::auto_play`) |
| Card generation / cloning | Undeath (clone); CaptureSpirit, Dirge, GraveWarden, Reave, Severance (created) | done (`create_card`, `create_clone`, `add_generated_card_to_combat`) |
| `IsPlayable` | HighFive | done |
| History: cards drawn, plays finished (Ethereal), Osty attacks, Doom received, this turn / combat | DeathMarch; BansheesCry, PullFromBelow, Fetch; Flatten, Rattle; DeathsDoor | done |
| Card choices (`CardSelectCmd`) | Cleanse, Dredge, Graveblast, SculptingStrike, Seance, Snap, Transfigure | existing (`Engine::choose_cards`; override `may_prompt` to `true`) |
| `CardCmd.TransformTo` | Seance | done (`Engine::transform_to`) |
| Downgrade (`AfterDowngraded`) | TheScythe | done (`Engine::downgrade`; nothing in scope downgrades a card) |
| Card instance fields | TheScythe (`IncreasedDamage`) | done (`CardData`); the deck version's buff is run layer |
| Eternal | ForbiddenGrimoire | deferred: run layer only (`IsRemovable`) |
| Global keywords (`ModifyKeywordsInCombat`), `BeforeCardAutoPlayed` | no Act 1 listener | deferred: comment at the call site |
| `ModifyCardPlayResultLocation` (`ResultLocation { pile, position }`, plus the play's `Resources { energy, stars }`: `EnergyValue`/`StarValue`) | NostalgiaPower, FeralPower | done (C12, C23) |
| Sly (`CardCmd.Discard` autoplay) | none of the 86 | done (`Engine::discard`) |
| Dupes, star costs | none of the 86 | deferred: no in-scope user |

## Relics
- Combat hooks are `Hooks` methods, as for every model. Fields (`[SavedProperty]` and private) are a
  `RelicData` variant from `init_data`; in combat read them with `s.relic(me).data`, write them with
  `s.relic_mut(me)`; they outlive the combat (the run stores them in `RunRelic::data`), so reset them only
  where the C# does.
- Hooks that fire outside combat are `RelicModel` methods (the run section of the trait in
  `content/relics/mod.rs`), named after the C# hook; the run layer dispatches them over the owned relics in
  order (`run/hooks.rs`). `me` is the relic's index in `s.relics`, so its fields are `s.relics[me].data`. Most
  take `&mut RunState` (or `&RunState` for `Modify*`/`Should*`) and use the run commands below. A hook the
  combat engine also dispatches during a fight has an `_out_of_combat` twin (`after_room_entered_out_of_combat`,
  `after_gold_gained_out_of_combat`, `modify_gold_gained_out_of_combat`, ...): implement both when the C#
  acts in both. `after_combat_end_on_deck` is the part of `AfterCombatEnd` that acts on the deck (the combat
  does not see it), run after the fight.
- `after_obtained(ctx, me)` is `AfterObtained`; `ctx` is a `RunCtx`: `ctx.state()`, `ctx.select(purpose,
  options, count)` (`FromDeckForUpgrade`/`ForRemoval`/`ForEnchantment` via `SelectPurpose::{Upgrade, Remove
  { damage }, Enchant { kind, amount }}` and `RunState::{upgradable,removable,enchantable}_cards`),
  `ctx.offer_rewards`, `ctx.create_cards`, `ctx.pull_relic_from_front`, `ctx.obtain_relic`, `ctx.remove_relic`,
  `ctx.replace_relic`. At most one prompt per hook.
- `TryModifyRewards` gets the populated rewards and pushes what it adds as `PendingReward`s (`Ready(Reward)`
  for a reward populated at construction such as `new GoldReward(15)`, `Card(CardCreation, count)`, `Relic`,
  `Potion`, `Gold(min, max)`); they are populated after every listener ran.
- `obtainable()` returns `false` while something the relic does is not ported (the run then refuses it).
- `after_card_added_to_deck(s, me, card)` is `AfterCardChangedPiles` for a card that joined the deck
  (`relics/lucky_fysh.rs`).
- More run hooks (RB): `modify_merchant_card_creation_results` (eggs; also re-run after every purchase),
  `try_modify_rest_site_options` (push a `RestOption { kind: RestOptionKind::Lift | Dig, .. }`; Dig/Lift are run in
  `Run::choose_rest_option`), `try_modify_rest_site_heal_rewards` (pushes `PendingReward`s the heal offers),
  `after_map_combat_room_entered` (the run-state half of a fight room's `AfterRoomEntered`, before the fight is set up:
  `Planisphere`, `MawBank`), `after_item_purchased(s, me, gold_spent)` (after each shop purchase and its restock, and
  after a shop removal: `MawBank`), `after_combat_victory_in_run(ctx, me, room)` (`RelicCmd.Replace` after a win: `SwordOfStone`).
  `try_modify_card_reward_options(_late)` take `&mut RunState` and `&mut Vec<RunCard>` (a hook may add a card, or draw:
  `LastingCandy`, `WingCharm`); a relic obtained from a rewards screen also runs them on the cards still shown
  (`CardReward.OnRelicObtained`). `CardCreation::flags` carries `IsFromCombat`, `NoCardPoolModifications`,
  `NoHookUpgrades`, `NoModifyHooks`; `rewards::pick_card` is the pick of `CreateForReward` for a hand-made pool.
- Prompts for `AfterObtained`: `ctx.select(purpose, options, count)` (exactly `count` picks), `ctx.select_up_to(purpose,
  options, max)` (`MinSelect` 0, confirmed with `RunAction::ConfirmSelect`; `SelectPurpose::EnchantUpTo`), `SelectPurpose::Clone`
  (`DollysMirror`), and `ctx.then_add_curses(n)` for what follows the prompt (`NeowsBones`).
- `is_allowed(s)` is `IsAllowed(runState)`: the relic bags cull a relic that fails it before every pull
  (`run/relic_bag.rs`; the shared bag also refills an emptied bucket). Most overrides are
  `relic_bag::is_before_act3_treasure_chest(s)` (`relics/girya.rs`). A pool relic with an override must have a file
  (`relic_bag::tests::every_culled_pool_relic_is_ported`); pull through `s.with_relic_bag(Bag::Player, |bag, s| ..)`.
- Map hooks (C7): `modify_generated_map(s, me, map, act_index)` (`ModifyGeneratedMap`: replace with `*map = ..`, e.g.
  `ActMap::golden_path()` for `GoldenCompass`), `modify_generated_map_late` (`ModifyGeneratedMapLate`, after every
  listener's early pass: read the final map, write the relic's own data, e.g. `FurCoat`'s marked coords) and
  `modify_unknown_map_point_room_types(s, me, types)` (the `?` room types: `GoldenCompass` sets `[Event]`). They run
  whenever the map is generated: entering an act, and `ctx.regenerate_map()` from `after_obtained`
  (`RunManager.Instance.GenerateMap()`), which also drops visited coords the new map lacks. A deck `SpoilsMap` is the
  card precedent (`run/hooks.rs`). Copy the test-only `relics/test_golden_path.rs`, `relics/test_map_marker.rs`,
  `relics/test_unknown_shops.rs` (tests in `run/tests_map_hooks.rs`). Map quest markers (`AfterMapGenerated`,
  `MapPoint.Quests`) are not modelled.
- Prove it with `tools/diff_run.sh <n>` (the policy takes it where a reward, treasure or Neow offers it) and a
  unit test in `run/tests.rs` or `run/tests_rb.rs` (`with_relics(&[..])`).

## Monsters
- Per-combat fields (C# private fields such as `_isAlone`) go in the struct, which is the payload of its
  `MonsterKind` variant (`MonsterKind::Nibbit(Nibbit { .. })`); stateless monsters use a unit struct
  (`MonsterKind::Osty(Osty)`).
- `moves()` returns a `static MoveTable`: the states of `GenerateMoveStateMachine` as `StateDef::Move`
  (id, follow-up), `StateDef::Random` (`Branch::new(state, Repeat::...)`, `Branch::weighted` for a
  constant `f32` weight, or `Branch::dynamic(state, cooldown, repeat, fn(&CombatState, CreatureId) -> f32)`
  for a weight lambda that reads the combat, `monsters/two_tailed_rat.rs`) and
  `StateDef::Conditional` (`fn(&CombatState, CreatureId) -> bool`), indexed by `const` state indices,
  plus the initial state. The generic machine (`monsters/machine.rs`) does `RollMove`, the state log,
  repeat/cooldown rules and the `MonsterAi` draw; never draw it yourself.
- Lifecycle: `after_added_to_room` (async) and `before_removed_from_room` (sync, a dead enemy leaving).
- Summons: `eng.add_enemy(kind, Some(slot))` (`CreatureCmd.Add`). A power with `owner_is_secondary_enemy`
  (Minion) makes its owner secondary: it dies with the last primary enemy and does not keep combat going.
- A move built outside the table and forced with `SetMoveImmediate` is a `machine::ForcedMove` variant
  (`IllusionPower`'s revive), dispatched in `monsters::perform_move`.
- `perform_move(eng, me, state)` matches the state index and runs the move; helpers:
  `attack_players(eng, me, dmg)`, `add_slimed(eng, n)`, `eng.apply_power(..)`, `eng.gain_block(..)`,
  and `if_ascension(asc, TOUGH_ENEMIES | DEADLY_ENEMIES, then, else)` for `AscensionHelper`.
- Intents are not declared: `CombatState::intent_damage` runs the move on a scratch copy and reads its
  attacks. The one thing a move declares is the type of its attack intent: `IntentType` (`combat/intent.rs`,
  `Attack` or `DeathBlow`). A move built with `DeathBlowIntent` (`GasBomb`, `WaterfallGiant`, both Underdocks)
  overrides `MonsterModel::attack_intent_type(state)` to return `IntentType::DeathBlow` for its state index;
  `CombatState::intends_to_attack` (`MonsterModel.IntendsToAttack`: Attack or DeathBlow) then counts it.

## Underdocks mechanics
Underdocks (`docs/specs/underdocks.md`, batches in `../history/underdocks-batches.md`) is the alternate Act 1.
- Act data: `run/rooms.rs::UNDERDOCKS` lists all 20 encounters (room, `IsWeak`, `Tags`) and 10 events
  whether ported or not, because room generation draws over the whole list. Porting an encounter
  or event is therefore just its file. A new event-only encounter (`PUNCH_OFF_EVENT_ENCOUNTER` is
  listed) needs one line in `EVENT_ENCOUNTERS`. An unported event is refused (`run/events.rs::is_allowed`);
  its `IsAllowed` comes with its file.
- Act 1 choice: `Run::new(.., act1)` takes `Act1::{Random, Overgrowth, Underdocks}` (the lobby's
  `Act1`; `Random` rolls it like the real game, `rooms::select_acts`). The harness always forces it:
  `ACT1=underdocks` for `tools/diff_run.sh` (`run-gen --act1`), `"act1"` in a run loadout, and
  `"acts"` in a run loadout or `ACTS=` for `tools/regress_runs.sh`. Forced fights (`tools/diff.sh`)
  don't depend on the act.
- Summons: `eng.add_enemy(kind, slot)` (`CreatureCmd.Add<T>`) creates and adds in one step. When the
  C# creates first and adds later (`CombatState.CreateCreature` then `CreatureCmd.Add(creature)`,
  which `SurprisePower` uses to give the fat gremlin its `HeistPower`s before it joins), use
  `eng.create_enemy(kind, slot)` (HP rolled on `Niche` and the `CombatId` taken now, not in combat
  yet) and later `eng.add_created_enemy(id)`. `s.encounter.next_slot(s)` is `GetNextSlot` (the first
  free slot of `Slots`, or `None`). For the last free slot (`TwoTailedRat`), filter `slots()` yourself.
  Put `Minion` on the summon where the C# does (`GasBomb.AfterAddedToRoom`).
- Escape: `eng.escape(id)` (`CreatureCmd.Escape`) drops the creature's powers without callbacks,
  runs `before_removed_from_room` and records it in `s.escaped`. No death hooks run. The monster's
  move ends and the turn loop checks for a win as usual.
- Gold: `EncounterModel::calculate_gold_proportion(&self, s) -> GoldProportion` (default
  `1 - escaped / spawned models`, from `s.escaped` and `s.spawned_enemies`) runs when the fight is won
  and scales a monster room's gold reward (`run/rewards.rs`). Return `GoldProportion::{ONE, HALF,
  ZERO}` from an override; never compute floats in a content file. An encounter's own fields
  (`GremlinMercNormal.GoldWasStolen`) go in a new `EncounterData` variant in `s.encounter_data`
  (`encounters/mod.rs`), which a power sets (`SurprisePower.MarkGoldStolen`).
- Wake-up: `AsleepPower` needs nothing new. On unblocked damage it removes `Plating` and calls
  `monsters::stun(eng, owner, Some(SLASH_MOVE))` (copy `CeremonialBeast` / `PlowPower`). At turn
  end it decrements and removes itself. The Matriarch's half-HP `IsShellAwake` is animation only.
  The `SLEEP_BRANCH` conditional reads `has_power(Asleep)`.

## Run-deck theft (C6)
`SwipePower` (`ThievingHopper.ThieveryMove`) takes a card out of the run deck during a fight. Copy
`powers/test_swipe.rs` (a test-only `SwipePower`).
- `eng.remove_from_combat(&[card])` is `CardPileCmd.RemoveFromCombat`: the cards leave their piles,
  then `AfterCardChangedPiles` per card; the `CardId` stays valid (store it as the power's `StolenCard`).
- `eng.remove_from_deck(deck_index)` is `CardPileCmd.RemoveFromDeck(card.DeckVersion)`; pass the combat
  card's `deck_version` (skip it when `None`). It is recorded in `s.removed_from_deck`; the run removes
  those cards (with `BeforeCardRemoved`) when the fight ends, win or loss.
- `BeforeDeath` returns the card with `eng.s.extra_rewards.push(ExtraReward::SpecialCard(deck_index))`
  (`SpecialCardReward`; `runState.AddCard` only re-registers the card, it is not in the deck). The run
  offers `Reward::SpecialCard(RunCard)` (the deck card with its end-of-fight counter), sorted by
  `RewardsSetIndex` 4 (after relics, before card rewards); `ClaimReward` adds it with `add_card`
  (`CardPileCmd.Add(card, Deck)`); skipping it loses the card. An escape (`eng.escape`) drops the power
  with no callback, so the card stays gone.
- The referee (sts2-cli) auto-claims only gold, relic and potion rewards, so in diffs a returned card is
  never taken (`run_session.rs` leaves it). History (`MarkLootStolen`/`MarkLootReturned`, `CardsRemoved`)
  is not modelled.

## Potions
- `name`, `usage` (`PotionUsage`), `target` (`TargetType`: `AnyEnemy`, `AnyPlayer`, `Myself`, `None`) and
  `on_use(eng, target)`, the body of `OnUse`. `target` was checked by `IsValidTarget` and is `Some` for
  every single-target type (`AssertValidForTargetedPotion` is the `expect`). Read `DynamicVars` values as
  consts with the var in the doc comment (`DamageVar(20m, ValueProp.Unpowered)`), and pass the var's
  props through (`ValueProps::UNPOWERED`).
- An `AnyTime` potion also implements `on_use_out_of_combat(&mut RunState)`, its `OnUse` on the player
  outside combat (`RunAction::UsePotion`; copy `potions/fruit_juice.rs`). An unported potion still holds its
  belt slot in combat (`Player::unported_potions`).
- The engine does the rest (`combat/potion_use.rs`, `OnUseWrapper`): empties the slot, `BeforePotionUsed`,
  effect depth, `AfterPotionUsed` if the player lives, `CheckWinCondition`. Potions also listen to hooks
  (after the relics, by slot) like any other model.
- Commands potions use beyond the list below: `gain_energy`. Prove it with
  `LOADOUT=<file> tools/diff.sh <n> <Encounter>` where the file is `{"potions": ["FIRE_POTION"]}`
  (the random policy uses potions; needs the referee's loadout support).

## Events
Events act on the run, not on a combat: one file per event in `content/events/`, a unit struct implementing
`EventModel` (registered by its file, see Registration). The
framework (`run/events.rs`) picks the event (`PullNextEvent` with every list event's `IsAllowed`), creates its
`Rng` (`runSeed + xxh64(Id.Entry)`), counts the visit, and leaves for the map once the event is finished.
- `EventKind::id()` is `Id.Entry`; `is_allowed(&RunState)` is the `IsAllowed` override (default `true`), read before the
  event is picked, so it sees canonical vars. Act gates go there too (`CurrentActIndex` is `s.act_index`, 0-based):
  every act's event list (`run/rooms.rs`) already holds every event, so a new event, of any act, is its file only.
  An unported event is refused (`run/events.rs::is_allowed`); copy `events/test_act_gated.rs` for a gate.
- `begin(ctx)` is `BeforeEventStarted` + `CalculateVars` + `GenerateInitialOptions`: roll vars on `ctx.rng`
  (the event `Rng`), store what later pages need in `*ctx.data` (an `EventData` variant; add one for a new
  shape), and return the options. `EventOption::new(text_key, tag)` takes the full `TextKey` (the referee
  compares it) and a tag naming the callback; `with_arg` adds a `usize` (a potion slot); `locked(text_key)` is
  an option with a null `onChosen`.
- `choose(action, ctx)` is the callback: match `action.tag`, mutate the run, and return `Page::Next(options)`
  (`SetEventState`) or `Page::Finished` (`SetEventFinished`). An error (an unported relic, potion) rolls the
  whole option back.
- Run commands (`ctx.state()` is the `RunState`; they run the run-level hooks): `gain_gold(Decimal)`
  (`ModifyGoldGained`, `(int)` cast, only if positive, `AfterGoldGained`), `lose_gold`, `damage` (unblockable,
  out of combat), `heal(Decimal)`, `gain_max_hp`, `lose_max_hp`, `kill`, `add_card` (`ModifyCardBeingAddedToDeck`),
  `remove_card(deck index)` / `remove_cards(indices)` (`BeforeCardRemoved`), `transform_to_random(index, rng)`,
  `transform_to(index, card)`, `downgrade`, `enchant(index, kind, amount)`, `discard_potion(slot)`,
  `procure_potion`, `gain_max_potion_count`, `potions_held()`, and the deck lists
  `{upgradable,removable,transformable,enchantable}_cards()` (in the order the game's select screens list
  them); deck card facts `RunCard::{is_basic, is_removable, card_type, is_upgradable, can_enchant}`. On `ctx`
  (an `EventCtx` derefs to the `RunCtx` above): `obtain_relic(id)?` (`RelicCmd.Obtain`; an unported relic is an
  error), `remove_relic`, `replace_relic`, `pull_relic_from_front()`, `create_cards(count, CardCreation)`
  (`CardFactory.CreateForReward`; build options with `CardCreation::{non_combat_default,
  non_combat_uniform}(pool, ..)`, `.as_card_reward()` for a `CardReward`), `offer_rewards(Vec<Reward>)`
  (`RewardsCmd.OfferCustom`), `select_cards_to_add(cards, count)` (`SelectCardsToAddToDeckFromGrid`),
  `select(purpose, options, count)` (a deck selection the run applies), and, for events only,
  `select_deck_cards(action, options, count)`: the event's own `FromDeck*` selection, whose picks come back in
  `EventModel::on_cards_selected(action, ctx, cards)` (deck indices before any change, in pick order; copy
  `events/wellspring.rs` for a removal, `events/morphic_grove.rs` for a transform on `ctx.transform_to_random`),
  and `enter_combat(encounter, extra_rewards, resume)` (`EnterCombatWithoutExitingEvent`: the fight's rewards
  are the room's plus the extras; the event resumes after them if `resume`). `EventModel::resume(exited, ctx)` is
  `Resume(exitedRoom)`: it runs when the event comes back (`ExitedCombat`: the encounter, its `EncounterData`, the
  room's `ExtraRewards`), may replace the page and open one prompt (`events/battleworn_dummy.rs`). At most one prompt per option; the
  event resumes after it, and effects after a `select_deck_cards` belong in `on_cards_selected`. An option whose page depends
  on what a prompt did (`EndlessConveyor`) returns a placeholder page, calls `ctx.then(action)`, and builds
  the real page in `EventModel::after_prompt(action, ctx)`, which runs once its prompts are done (or at once
  when it opened none). A
  `(decimal)float` is `run::odds::decimal_from_f32`.
- A player killed by an option ends the run there. Prove an event with `tools/diff_run.sh <n>` (it counts the
  event rooms entered); an option that throws in the referee goes in `REFEREE_BROKEN_OPTIONS`
  (`crates/sts2-diff/src/run_session.rs`).

## Ancients
An ancient (`AncientEventModel`) is one file in `content/ancients/`, a unit struct implementing `AncientModel`
(registered by its file; copy `ancients/test_ancient.rs`, or `ancients/neow.rs`). The file declares its option pools
and ports `GenerateInitialOptions` as `generate_options(&RunState, &mut Rng) -> Vec<AncientOption>`: `rng` is the
event `Rng` (`runSeed + xxh64(Id.Entry)`), `s` the run as entered (`Owner.Deck`, `Owner.Character`; already healed).
Options: `AncientOption::relic(id)` (`RelicOption<T>()`), `AncientOption::Relic { id, data: Some(RelicData) }` for a
relic whose fields are set before `Obtain` (`SeaGlass.CharacterId`, `SetupForPlayer`; add the relic's `RelicData`
variant), `AncientOption::Locked(name)` for a null `onChosen` (`OROBAS...OPTION_POOL_3_LOCKED`). The text key is
`<ANCIENT>.pages.INITIAL.options.<relic or name>`.
The framework (`Run::begin_ancient`) does the rest: the act's ancient is pre-rolled (`run/rooms.rs::generate_rooms`:
the act's `ancients` plus its share of `SHARED_ANCIENTS`), Act 1's start point enters it when the run begins and
acts 2-3 enter it when the player travels to the start point (`Room::Ancient { id }`, `RunDecision::Ancient`,
`RunAction::AncientOption(i)`); `BeforeEventStarted` heals the missing HP (×0.8 from `WearyTraveler`; Neow first
sets HP to 0); the chosen relic is obtained (`AfterObtained` may prompt), then the player is on the map. An
unported ancient is a stub (`RunDecision::Unsupported(Ancient)`); an unported relic option is
`RunError::UnsupportedAncientOption`. Relics an ancient offers are ordinary relic files. `CalculateVars`, dialogue and
`ShouldAllowAncient` (no override in the game) are not modeled.

## Rules
- Per-card data (`CardData`): when a card reads its own variant, match it plus `CardData::None`, and use one `_ => unreachable!(..)` arm for everything else. Never list other cards' variants, or every new card breaks the existing ones.
- Interface tags from the decomp are trait flags you must copy: `ITemporaryPower` → `PowerModel::internally_applied_power` (`PowerKind::is_temporary`: Illusion keeps temporary debuffs on death; the `Temporary{Strength,Dexterity}Power` subclasses forward to `powers/temporary.rs`, copy `powers/flex_potion.rs`), `IsVisibleInternal` → `PowerModel::is_visible` (`powers/ambergris.rs`), `OstyAttack` tag → `CardModel::is_osty_attack`. Check the C# class declaration for interfaces and tags. `content/flags.rs` pins these flags (and type, rarity, target, costs, keywords, tags, `GainsBlock`, `InstanceType`, pools and the pool tables) for every registered entity against `content/flags/*.tsv`; regenerate those with `python3 tools/flag_truth.py` (ids and values only).
- Player choices: a model that can raise one (`eng.choose_cards`), directly or by creating, transforming
  into or playing a model that can, overrides its kind trait's `may_prompt()` to return `true`
  (`cards/seance.rs`, `potions/ashwater.rs`). The combat only snapshots a step for replay while such a model is in
  combat; a missing override panics at the choice in every build (C23: debug builds always snapshot for the replay
  check, which used to hide it), and debug builds re-run every step to check replay.
- Commands live on `Engine` (`combat/commands/`): `attack`, `damage`, `kill`, `gain_block`, `heal`,
  `set_max_hp`, `gain_max_hp`, `gain_energy`, `apply_power`, `apply_power_instance` (returns the
  instance `PowerCmd.Apply<T>` returns, for `SetDamage`-style writes), `apply_power_to_all`, `apply_power_from_card` /
  `apply_power_to_all_from_card` (pass the playing card as `cardSource`: every card that applies a power to an enemy
  does, `UnsettlingLamp` reads it), `modify_power_amount`,
  `decrement_power`, `remove_power`, `draw(Decimal, from_hand_draw)`, `shuffle`, `add_to_pile`,
  `add_cards_to_pile` (one batch; `Position::{Bottom, Top, Random}`, `Random` draws `Rng.Shuffle`), `exhaust(card, caused_by_ethereal)`,
  `add_to_combat_and_preview(target, kind, pile, count, creator)`, `create_card`, `create_clone`,
  `add_generated_card_to_combat` (`_at` takes a `Position`; `add_generated_cards_to_combat` for a list), `upgrade_card`, `apply_keyword`, `remove_keyword`,
  `auto_play(card, target)`, `summon_osty`, `add_enemy(kind, slot)` (`CreatureCmd.Add`), `afflict`,
  `clear_affliction`, `enchant`, `downgrade`, `transform_to`, `discard` (with Sly autoplay),
  `try_to_procure_potion`, `discard_potion`, `gain_gold`, `lose_gold` (the combat's copy of
  `Player.Gold`), `lose_energy`, `lose_block`, `discard_and_draw(cards, n)`, `auto_play_from_draw_pile(count, Position, force_exhaust)`,
  `shuffle_if_necessary`, `get_distinct_for_combat(&pool, count)` (`CardFactory`, on
  `CombatCardGeneration`; pools in `content/cards/pools.rs`: `character(c)`, `colorless()`, filter
  with `PoolCard` fields; returns `Flow<Vec<CardId>>`: an unported pick stops the combat with
  `Unsupported`, so propagate it with `?`; content that meets anything else unported returns
  `Unsupported::card(id).stop()` (`combat::Unsupported`, `ContentKind::{Card, Potion, Relic}`)) and `choose_a_card(cards)`
  (`FromChooseACardScreen`; override `may_prompt`). Propagate their `Flow` with `?`. Dispatchers a model
  calls itself: `after_died_to_doom` (`DoomPower`); value folds on `CombatState`: `max_energy`,
  `energy_cost`, `energy_to_spend`, `resolve_energy_x_value`, `hittable_enemies`.
- The pile of a card is `eng.s.pile_of(card)`. Never push to or remove from `player.hand`/`draw`/... directly:
  card moves go through the commands (`add_to_pile`, ...), which keep `pile_of` and the engine's cached
  listener list in step (core code: `CombatState::{remove_from_pile, insert_into_pile}` plus
  `Engine::listeners_changed`; debug builds check both).
- Hooks outside the Act 1 inventory may still lack a call site; they are marked in the engine with a
  `// Hook.X: no in-scope listener.` comment, and adding the first listener means adding the call there
  (a core change: report it).
- No floats outside `rng/`; `decimal` values are `Decimal` (`Decimal::new(7, 1)` is `0.7m`).
- Facts you learn go in the matching `docs/semantics/<topic>.md` (index: `docs/semantics.md`); prove the port with `tools/diff.sh <n> <Encounter>`.

## Hooks
The full hook table (when each fires, call site, decomp reference, status) is in `docs/specs/hooks.md`. Look a hook up there by name.

## Test mode
If an entity's decomp checks `TestMode.IsOn`/`IsOff` (see `docs/specs/test-mode.md`), port both branches behind `Run::in_test_mode` (or the combat equivalent) and unit-test the real-game branch.
