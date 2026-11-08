//! Plain-data combat state: ids instead of references, every RNG stream it touches inline.

use super::DeckCard;
use super::cost_lists::CostLists;
use super::energy_cost::EnergyCost;
use super::history::CombatHistory;
use super::hooks::HookSet;
use super::listeners::HookIndex;
use super::star_cost::StarCost;
use crate::content::afflictions::Affliction;
use crate::content::cards::{self, CardData, CardKeyword, CardKind, Keywords};
use crate::content::characters::Character;
use crate::content::enchantments::Enchantment;
use crate::content::encounters::Encounter;
use crate::content::encounters::EncounterData;
use crate::content::monsters::Monster;
use crate::content::potions::PotionKind;
use crate::content::powers::{PowerData, PowerKind};
use crate::content::relics::{RelicData, RelicKind};
use crate::decimal::{Decimal, PackedDecimal};
use crate::rng::RunRngSet;
use crate::run::RoomType;
use crate::run::odds::GoldProportion;
use smallvec::SmallVec;

/// A short list of creatures, inline up to a full side.
pub type Creatures = SmallVec<[CreatureId; 8]>;

/// Cards with the pile each left, as a batch move fills them in.
pub(crate) type MovedCards = SmallVec<[(CardId, Option<PileType>); 16]>;

/// A creature's `CombatId`; also its index in [`CombatState::creatures`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CreatureId(pub u32);

/// A combat card instance; also its index in [`CombatState::cards`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CardId(pub u32);

/// A power instance, unique for the whole combat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PowerId(pub u32);

/// A relic instance; also its index in [`Player::relics`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelicId(pub u32);

/// A potion instance, unique for the whole combat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PotionId(pub u32);

/// An orb instance (`OrbModel`), unique for the whole combat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrbId(pub u32);

impl_index!(CreatureId, CardId, PowerId, RelicId, PotionId, OrbId; id);
impl_index!(PileType, OrbKind);

/// `CombatSide`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    /// The player and their pets.
    Player,
    /// The monsters.
    Enemy,
}

/// The combat card piles (`PileType`), in the order hook listeners visit them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PileType {
    /// The hand.
    Hand,
    /// The draw pile.
    Draw,
    /// The discard pile.
    Discard,
    /// The exhaust pile.
    Exhaust,
    /// The Play pile.
    Play,
}

impl PileType {
    /// Listener order of the piles (`PlayerCombatState.cs:70-80`).
    pub const LISTENER_ORDER: [Self; 5] = [
        Self::Hand,
        Self::Draw,
        Self::Discard,
        Self::Exhaust,
        Self::Play,
    ];
}

/// `PlayerTurnPhase`, in enum order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TurnPhase {
    /// Outside a turn.
    None,
    /// The turn is starting.
    Start,
    /// `CombatState.AutoPrePlay`.
    AutoPrePlay,
    /// The player acts.
    Play,
    /// `CombatState.AutoPostPlay`.
    AutoPostPlay,
    /// The turn is ending.
    End,
}

/// How a combat ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CombatResult {
    /// Every enemy died or left.
    Won,
    /// Every player died.
    Lost,
}

/// A power instance on a creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Power {
    /// The instance's id.
    pub id: PowerId,
    /// The power.
    pub kind: PowerKind,
    /// `Amount`.
    pub amount: i32,
    /// `AmountOnTurnStart`: snapshot taken at the start of each of the owner's side's turns.
    pub amount_on_turn_start: i32,
    /// The creature that applied it, if any.
    pub applier: Option<CreatureId>,
    /// `SkipNextDurationTick`.
    pub skip_next_duration_tick: bool,
}

/// `_internalData` (the per-instance data `InitInternalData` creates) of each power that has any.
///
/// Keyed by [`PowerId`], for the powers whose data is not `None`. It sits beside the creatures,
/// not in [`Power`], so that a power is 24 bytes instead of 176: a search copies every
/// creature's powers for each candidate action, and few powers carry data. A removed instance
/// keeps its entry (a model still running reads it). A power without an entry has `None`, and
/// `None` entries compare and hash as absent, so the table is the same however it was reached.
#[derive(Debug, Default)]
pub struct PowerDataTable(Vec<(PowerId, PowerData)>);

impl Clone for PowerDataTable {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    fn clone_from(&mut self, source: &Self) {
        crate::copy_vec_from(&mut self.0, &source.0);
    }
}

impl PowerDataTable {
    /// The entries other than `None` ([`PowerDataTable::get_mut`] may leave a `None` entry).
    fn entries(&self) -> impl Iterator<Item = &(PowerId, PowerData)> {
        self.0.iter().filter(|(_, d)| *d != PowerData::None)
    }
}

impl PartialEq for PowerDataTable {
    fn eq(&self, other: &Self) -> bool {
        self.entries().eq(other.entries())
    }
}

impl Eq for PowerDataTable {}

impl std::hash::Hash for PowerDataTable {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for entry in self.entries() {
            entry.hash(state);
        }
    }
}

impl PowerDataTable {
    /// The data of the power `id`.
    #[must_use]
    #[inline]
    pub fn get(&self, id: PowerId) -> &PowerData {
        static NONE: PowerData = PowerData::None;
        if self.0.is_empty() {
            return &NONE;
        }
        self.0
            .binary_search_by_key(&id, |&(p, _)| p)
            .map_or(&NONE, |i| &self.0[i].1)
    }

    /// The data of the power `id`, mutably (an entry is made if it has none).
    pub(crate) fn get_mut(&mut self, id: PowerId) -> &mut PowerData {
        let i = match self.0.binary_search_by_key(&id, |&(p, _)| p) {
            Ok(i) => i,
            Err(i) => {
                self.0.insert(i, (id, PowerData::None));
                i
            }
        };
        &mut self.0[i].1
    }

    /// Sets the data of the power `id`.
    pub(crate) fn set(&mut self, id: PowerId, data: PowerData) {
        match self.0.binary_search_by_key(&id, |&(p, _)| p) {
            Ok(i) if data == PowerData::None => {
                self.0.remove(i);
            }
            Ok(i) => self.0[i].1 = data,
            Err(_) if data == PowerData::None => {}
            Err(i) => self.0.insert(i, (id, data)),
        }
    }
}

/// Words of [`PowerKinds`].
const POWER_KIND_WORDS: usize = PowerKind::ALL.len().div_ceil(64);

/// The power kinds a creature has, one bit per [`PowerKind`]: kept in step with
/// [`Creature::powers`], so `HasPower` and the combat-end checks test a bit instead of scanning.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PowerKinds([u64; POWER_KIND_WORDS]);

impl PowerKinds {
    /// The kinds of `powers`.
    fn of(powers: &[Power]) -> Self {
        let mut kinds = Self::default();
        for p in powers {
            kinds.insert(p.kind);
        }
        kinds
    }

    #[inline]
    const fn insert(&mut self, kind: PowerKind) {
        let i = kind.index();
        self.0[i / 64] |= 1 << (i % 64);
    }

    #[inline]
    const fn remove(&mut self, kind: PowerKind) {
        let i = kind.index();
        self.0[i / 64] &= !(1 << (i % 64));
    }

    /// Whether `kind` is in the set.
    #[must_use]
    #[inline]
    pub const fn contains(self, kind: PowerKind) -> bool {
        let i = kind.index();
        self.0[i / 64] & (1 << (i % 64)) != 0
    }
}

/// A player, enemy or pet.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Creature {
    /// The creature's id.
    pub id: CreatureId,
    /// Its side.
    pub side: Side,
    /// `CurrentHp`.
    pub hp: i32,
    /// `MaxHp`.
    pub max_hp: i32,
    /// `Block`.
    pub block: i32,
    /// In application order. Add and remove powers with the [`Creature`] methods, which keep
    /// [`Creature::power_kinds`] in step.
    pub powers: Vec<Power>,
    /// The kinds of [`Creature::powers`].
    pub(crate) power_kinds: PowerKinds,
    /// The hooks of its powers and monster ([`super::listeners`]), kept in step by the same
    /// methods as [`Creature::power_kinds`].
    pub(crate) hooks: HookSet,
    /// The hooks of its monster (none without one), so that a walk reads them without the
    /// generated `match`. Set with [`Creature::hooks`]; a monster does not change in combat.
    pub(crate) monster_hooks: HookSet,
    /// The instances removed from this creature, in removal order. A C# `PowerModel` outlives its
    /// removal: a model still running (or holding it) reads and writes its fields, and its `Owner`
    /// stays (`PowerModel.RemoveInternal`, `PowerModel.cs:583`). They no longer listen.
    pub removed_powers: Vec<Power>,
    /// `None` for the player creature.
    pub monster: Option<Monster>,
    /// The owning player's creature, for pets such as Osty.
    pub pet_owner: Option<CreatureId>,
    /// `SlotName`: the encounter slot an enemy was created in, if any.
    pub slot: Option<&'static str>,
    /// `HpDisplay.IsInfinite()`: set by `WaterfallGiant.TriggerAboutToBlowState` and
    /// `HardenedShellPower`; `HellraiserPower` reads it.
    pub infinite_hp_display: bool,
}

impl_clone_by_fields!(Creature {
    id,
    side,
    hp,
    max_hp,
    block,
    powers: copy,
    power_kinds,
    hooks,
    monster_hooks,
    removed_powers: copy,
    monster,
    pet_owner,
    slot,
    infinite_hp_display,
});

impl Creature {
    /// How many of the creature's powers and monster may prompt ([`CombatState::prompting`]).
    fn prompting(&self) -> u32 {
        let powers = self.powers.iter().filter(|p| p.kind.may_prompt()).count();
        let monster = self.monster.as_ref().is_some_and(|m| m.kind.may_prompt());
        u32::try_from(powers).expect("power count fits u32") + u32::from(monster)
    }

    /// The hooks of the creature's monster, from its kind ([`Creature::monster_hooks`]).
    pub(crate) fn count_monster_hooks(&self) -> HookSet {
        self.monster
            .as_ref()
            .map_or(HookSet::EMPTY, |m| m.kind.hooks())
    }

    /// The hooks of the creature's powers and monster, counted ([`Creature::hooks`]).
    pub(crate) fn count_hooks(&self) -> HookSet {
        self.powers
            .iter()
            .fold(self.monster_hooks, |hooks, p| hooks.union(p.kind.hooks()))
    }

    /// `IsAlive`: `CurrentHp > 0`.
    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.hp > 0
    }

    /// `IsDead`.
    #[must_use]
    pub const fn is_dead(&self) -> bool {
        !self.is_alive()
    }

    /// `IsPlayer`.
    #[must_use]
    pub const fn is_player(&self) -> bool {
        self.monster.is_none()
    }

    /// `IsSecondaryEnemy`: an enemy with a power whose `OwnerIsSecondaryEnemy` is set (Minion).
    /// A secondary enemy dies with the last primary one and does not keep combat going.
    ///
    /// Minion is the only such power (`secondary_enemy_powers_are_minion` pins it); the kind is
    /// compared directly because combat end checks ask this many times per action.
    #[must_use]
    #[inline]
    pub fn is_secondary_enemy(&self) -> bool {
        self.side == Side::Enemy && self.has_power(PowerKind::Minion)
    }

    /// `IsPrimaryEnemy`: an enemy that is not secondary.
    #[must_use]
    pub fn is_primary_enemy(&self) -> bool {
        self.side == Side::Enemy && !self.is_secondary_enemy()
    }

    /// The instance `id`, on the creature or already removed from it.
    pub(crate) fn power_instance(&self, id: PowerId) -> Option<&Power> {
        self.powers
            .iter()
            .chain(&self.removed_powers)
            .find(|p| p.id == id)
    }

    /// [`Creature::power_instance`], mutably.
    pub(crate) fn power_instance_mut(&mut self, id: PowerId) -> Option<&mut Power> {
        self.powers
            .iter_mut()
            .chain(&mut self.removed_powers)
            .find(|p| p.id == id)
    }

    /// `RemoveAllPowersInternalExcept` (`Creature.cs:663`): every power leaves the creature,
    /// without callbacks; the instances are kept as removed ones. The list keeps its buffer.
    pub(crate) fn remove_all_powers(&mut self) {
        self.removed_powers.extend_from_slice(&self.powers);
        self.powers.clear();
        self.power_kinds = PowerKinds::default();
        self.hooks = self.count_hooks();
    }

    /// Adds `power` after the creature's other powers.
    pub(crate) fn push_power(&mut self, power: Power) {
        self.power_kinds.insert(power.kind);
        self.hooks = self.hooks.union(power.kind.hooks());
        self.powers.push(power);
    }

    /// Takes the power at `index` off the creature (not into [`Creature::removed_powers`]).
    pub(crate) fn take_power(&mut self, index: usize) -> Power {
        let power = self.powers.remove(index);
        if !self.powers.iter().any(|p| p.kind == power.kind) {
            self.power_kinds.remove(power.kind);
        }
        if !power.kind.hooks().is_empty() {
            self.hooks = self.count_hooks();
        }
        power
    }

    /// Takes the powers for which `take` holds off the creature, in order (not into
    /// [`Creature::removed_powers`]).
    pub(crate) fn take_powers_where(&mut self, take: impl Fn(&Power) -> bool) -> Vec<Power> {
        // `retain`, not a partition into two new lists: the list keeps its buffer.
        let mut taken = Vec::new();
        self.powers.retain(|p| {
            let gone = take(p);
            if gone {
                taken.push(*p);
            }
            !gone
        });
        self.power_kinds = PowerKinds::of(&self.powers);
        self.hooks = self.count_hooks();
        taken
    }

    /// Whether the creature has a power of `kind` (`HasPower<T>`).
    #[must_use]
    #[inline]
    pub fn has_power(&self, kind: PowerKind) -> bool {
        let has = self.power_kinds.contains(kind);
        debug_assert_eq!(
            has,
            self.powers.iter().any(|p| p.kind == kind),
            "power kinds out of step"
        );
        has
    }

    /// The amount of the first power of `kind`, or 0.
    #[must_use]
    #[inline]
    pub fn power_amount(&self, kind: PowerKind) -> i32 {
        if !self.has_power(kind) {
            return 0;
        }
        self.powers
            .iter()
            .find(|p| p.kind == kind)
            .map_or(0, |p| p.amount)
    }
}

/// A combat card instance (a clone of a deck card) with its mutable `CardModel` state. Its cost
/// lists are in [`CombatState::cost_lists`], so a card is `Copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is an independent `CardModel` field (`ExhaustOnNextPlay`, `IsDupe`, ...)"
)]
pub struct Card {
    /// The card's id in this combat.
    pub id: CardId,
    /// The card model.
    pub kind: CardKind,
    /// Times upgraded.
    pub upgrade_level: u8,
    /// `LocalKeywords`: the canonical keywords, plus `AddKeyword`, minus `RemoveKeyword`.
    pub keywords: Keywords,
    /// `EnergyCost`.
    pub energy_cost: EnergyCost,
    /// `BaseStarCost`, the temporary star costs and `LastStarsSpent`.
    pub star_cost: StarCost,
    /// `ExhaustOnNextPlay`.
    pub exhaust_on_next_play: bool,
    /// `HasSingleTurnRetain`.
    pub single_turn_retain: bool,
    /// `HasSingleTurnSly`.
    pub single_turn_sly: bool,
    /// `Affliction`.
    pub affliction: Option<Affliction>,
    /// `Enchantment`.
    pub enchantment: Option<Enchantment>,
    /// `BaseReplayCount`: extra plays per play (before `Hook.ModifyCardPlayCount`).
    pub base_replay_count: u32,
    /// The card model's own per-instance fields.
    pub data: CardData,
    /// The combat pile holding the card (`Pile.Type`), kept in step with [`Player`]'s piles by
    /// the [`CombatState`] pile methods.
    pub(crate) pile: Option<PileType>,
    /// `IsDupe` (`CardModel.CreateDupe`): played once and removed from combat (`DupeOf` is
    /// [`Card::clone_of`]).
    pub is_dupe: bool,
    /// `CloneOf`: the card this one was cloned from (`CardModel.CreateClone`).
    pub clone_of: Option<CardId>,
    /// `DeckVersion`: the index in [`CombatState::deck`] of the deck card this combat card
    /// was cloned from at combat start (`Player.PopulateCombatState`); `None` for generated
    /// cards and clones (`AfterCloned`). Read it with [`Card::deck_version`]; a `u32`, not a
    /// `usize`, to keep the card small.
    pub(crate) deck_slot: Option<u32>,
    /// A calculated var's own `BaseValue` when it no longer matches `CalculationBase`:
    /// `DowngradeInternal` clones the new vars against the old ones, so `SetOwner` →
    /// `UpdateValues` reads the upgraded base (`CardModel.cs:2149-2162`,
    /// `CalculatedVar.cs:23-27,117-123`). Cleared by `RecalculateForUpgradeOrEnchant` (an upgrade
    /// or `ModifyCard`) and by a clone (`CardModel.cs:1202`). Only shown, never used: `Calculate`
    /// reads `CalculationBase`.
    /// Packed: a [`Decimal`] would align every card to 16 bytes.
    pub stale_calculated: Option<PackedDecimal>,
}

impl Card {
    /// How many of the card's model, affliction and enchantment may prompt
    /// ([`CombatState::prompting`]).
    fn prompting(&self) -> u32 {
        u32::from(self.kind.may_prompt())
            + u32::from(self.affliction.is_some_and(|a| a.kind.may_prompt()))
            + u32::from(self.enchantment.is_some_and(|e| e.kind.may_prompt()))
    }

    /// `DeckVersion`: the index in [`CombatState::deck`] of the deck card this combat card was
    /// cloned from at combat start; `None` for generated cards and clones.
    #[must_use]
    #[expect(clippy::as_conversions, reason = "const fn: `From` is not const")]
    pub const fn deck_version(&self) -> Option<usize> {
        match self.deck_slot {
            Some(i) => Some(i as usize),
            None => None,
        }
    }

    /// The deck card `deck` as a card instance: its upgrades, enchantment (with the keywords its
    /// `OnEnchant` gave) and saved fields, in no pile. A combat clone starts as this.
    #[must_use]
    pub fn from_deck(id: CardId, deck: DeckCard) -> Self {
        let mut card = Self::new(id, deck.kind, deck.upgrade_level);
        card.enchantment = deck.enchantment;
        crate::content::enchantments::modify_card(&mut card);
        card.data = cards::data_from_deck(deck.kind, deck.counter);
        card
    }

    /// A new instance of `kind` upgraded `upgrade_level` times (`ToMutable`, then
    /// `UpgradeInternal` per level, as the deck card a combat card is cloned from).
    #[must_use]
    pub fn new(id: CardId, kind: CardKind, upgrade_level: u8) -> Self {
        let mut card = Self {
            id,
            kind,
            upgrade_level: 0,
            keywords: kind.keywords(),
            energy_cost: EnergyCost::new(kind.cost(), kind.has_energy_cost_x()),
            star_cost: StarCost::new(kind.star_cost(), kind.has_star_cost_x()),
            exhaust_on_next_play: false,
            single_turn_retain: false,
            single_turn_sly: false,
            affliction: None,
            enchantment: None,
            base_replay_count: 0,
            data: CardData::None,
            pile: None,
            clone_of: None,
            is_dupe: false,
            deck_slot: None,
            stale_calculated: None,
        };
        for _ in 0..upgrade_level {
            card.upgrade_internal();
        }
        card
    }

    /// `Type`: virtual in `CardModel`, so an instance can differ from its canonical card
    /// (`MadScience` returns its `TinkerTimeType`, [`CardModel::instance_card_type`]).
    ///
    /// [`CardModel::instance_card_type`]: crate::content::cards::CardModel::instance_card_type
    #[must_use]
    pub fn card_type(&self) -> cards::CardType {
        cards::instance_card_type(self)
    }

    /// `GainsBlock` of this instance ([`Card::card_type`] explains why it is per instance).
    #[must_use]
    pub fn gains_block(&self) -> bool {
        cards::instance_gains_block(self)
    }

    /// `IsClone`: created by `CardModel.CreateClone`.
    #[must_use]
    pub const fn is_clone(&self) -> bool {
        self.clone_of.is_some()
    }

    /// Whether the card is upgraded at least once.
    #[must_use]
    pub const fn is_upgraded(&self) -> bool {
        self.upgrade_level > 0
    }

    /// `IsUpgradable`.
    #[must_use]
    pub fn is_upgradable(&self) -> bool {
        self.kind.is_upgradable(self.upgrade_level)
    }

    /// `UpgradeInternal` (no hooks) of the card itself: the next level, then `OnUpgrade`.
    /// Returns the bases `UpgradeBy` and `UpgradeStarCostBy` lowered the costs to, if they
    /// did, for the cost lists ([`CombatState::upgrade_card_internal`]).
    pub(crate) fn upgrade_internal(&mut self) -> (Option<i32>, Option<i32>) {
        self.upgrade_level += 1;
        cards::on_upgrade(self);
        self.stale_calculated = None;
        (
            self.energy_cost.upgrade_cap.take(),
            self.star_cost.upgrade_cap.take(),
        )
    }

    /// `LocalKeywords`: canonical, plus `AddKeyword`, minus `RemoveKeyword`. A combat card's
    /// `Keywords` also has the global ones from `Hook.ModifyKeywordsInCombat` (`HexPower`'s
    /// Ethereal): read those with [`CombatState::keywords`] / [`CombatState::has_keyword`].
    #[must_use]
    pub const fn local_keywords(&self) -> Keywords {
        self.keywords
    }

    /// `LocalKeywords.Contains(keyword)`.
    #[must_use]
    pub const fn has_local(&self, keyword: CardKeyword) -> bool {
        self.keywords.contains(keyword)
    }

    /// `EndOfTurnCleanup` (`CardModel.cs:1611`) of the card itself: clear
    /// `ExhaustOnNextPlay` and the single-turn retain and Sly. The turn-scoped costs are in
    /// the cost lists ([`CostLists::end_of_turn_cleanup`]).
    pub(crate) const fn end_of_turn_cleanup(&mut self) {
        self.exhaust_on_next_play = false;
        self.single_turn_retain = false;
        self.single_turn_sly = false;
    }
}

/// A reward a combat model adds to the room's rewards (`CombatRoom.AddExtraReward`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtraReward {
    /// `new GoldReward(amount, player)`.
    Gold(i32),
    /// `new CardReward(CardCreationOptions.ForRoom(owner, room.RoomType), count, owner)`.
    Card(usize),
    /// `new SpecialCardReward(card.DeckVersion, player)` (`Rewards/SpecialCardReward.cs`): the
    /// deck card at this index of [`CombatState::deck`], as it is when the fight ends. Picking
    /// it adds it back to the deck (`SwipePower.BeforeDeath` returns a stolen card this way).
    SpecialCard(usize),
}

/// A relic instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Relic {
    /// The relic's id in this combat.
    pub id: RelicId,
    /// The relic model.
    pub kind: RelicKind,
    /// The model's instance fields; they outlive the combat.
    pub data: RelicData,
    /// `IsMelted`: still owned (`GetRelic` finds it) but no hook listener.
    pub melted: bool,
}

/// A potion instance in a belt slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Potion {
    /// The potion's id in this combat.
    pub id: PotionId,
    /// The potion model.
    pub kind: PotionKind,
}

/// An orb kind (`Models.Orbs/*.cs`), in `OrbModel._validOrbs` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrbKind {
    /// `LightningOrb`.
    Lightning,
    /// `FrostOrb`.
    Frost,
    /// `DarkOrb`.
    Dark,
    /// `PlasmaOrb`.
    Plasma,
    /// `GlassOrb`.
    Glass,
}

impl OrbKind {
    /// `OrbModel._validOrbs` (`OrbModel.cs:27-34`), the list `GetRandomOrb` draws from.
    pub const ALL: [Self; 5] = [
        Self::Lightning,
        Self::Frost,
        Self::Dark,
        Self::Plasma,
        Self::Glass,
    ];

    /// `ModelId.Entry`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Lightning => "LIGHTNING_ORB",
            Self::Frost => "FROST_ORB",
            Self::Dark => "DARK_ORB",
            Self::Plasma => "PLASMA_ORB",
            Self::Glass => "GLASS_ORB",
        }
    }

    /// The orb's own field at creation: `DarkOrb._evokeVal = 6m`, `GlassOrb._passiveVal = 4m`
    /// (`DarkOrb.cs:17`, `GlassOrb.cs:15`); zero for the others.
    #[must_use]
    pub fn initial_value(self) -> Decimal {
        match self {
            Self::Dark => Decimal::from(6),
            Self::Glass => Decimal::from(4),
            Self::Lightning | Self::Frost | Self::Plasma => Decimal::ZERO,
        }
    }
}

/// A channeled orb (`OrbModel`); its behaviour is `combat/commands/orb.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Orb {
    /// The orb's id in this combat.
    pub id: OrbId,
    /// The orb.
    pub kind: OrbKind,
    /// The orb's own field: `DarkOrb._evokeVal`, `GlassOrb._passiveVal`; zero otherwise.
    pub value: Decimal,
}

/// A combat pile's cards: a `Vec<CardId>` (it derefs to one) whose clone keeps spare room.
///
/// A clone keeps room for [`Pile::CLONE_ROOM`] more cards. A clone of the state (a search copy)
/// goes on to play, and a pile cloned at its exact length reallocates (a `malloc`, a copy and a
/// `free`) on its next card; the hand and the discard pile grow on nearly every action.
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct Pile(Vec<CardId>);

impl Pile {
    /// Room a clone keeps.
    const CLONE_ROOM: usize = 4;
}

impl Clone for Pile {
    fn clone(&self) -> Self {
        if self.0.is_empty() {
            return Self(Vec::new());
        }
        let mut cards = Vec::with_capacity(self.0.len() + Self::CLONE_ROOM);
        cards.extend_from_slice(&self.0);
        Self(cards)
    }

    fn clone_from(&mut self, source: &Self) {
        crate::copy_vec_from(&mut self.0, &source.0);
    }
}

impl std::ops::Deref for Pile {
    type Target = Vec<CardId>;

    fn deref(&self) -> &Vec<CardId> {
        &self.0
    }
}

impl std::ops::DerefMut for Pile {
    fn deref_mut(&mut self) -> &mut Vec<CardId> {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a Pile {
    type Item = &'a CardId;
    type IntoIter = std::slice::Iter<'a, CardId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl IntoIterator for Pile {
    type Item = CardId;
    type IntoIter = std::vec::IntoIter<CardId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl PartialEq<[CardId]> for Pile {
    fn eq(&self, other: &[CardId]) -> bool {
        self.0 == other
    }
}

impl FromIterator<CardId> for Pile {
    fn from_iter<I: IntoIterator<Item = CardId>>(cards: I) -> Self {
        Self(cards.into_iter().collect())
    }
}

impl From<Vec<CardId>> for Pile {
    fn from(cards: Vec<CardId>) -> Self {
        Self(cards)
    }
}

/// The single player's combat state (`Player` plus `PlayerCombatState`). Every character has
/// every resource (orbs, stars, pets); unused ones stay zero or empty.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Player {
    /// `Player.Character`.
    pub character: Character,
    /// The player's creature.
    pub creature: CreatureId,
    /// Energy left this turn.
    pub energy: i32,
    /// `MaxEnergy` before `ModifyMaxEnergy`.
    pub max_energy: i32,
    /// `Player.Gold`: the run's gold at combat start, changed by `GainGold`/`LoseGold`.
    pub gold: i32,
    /// `PlayerCombatState.TurnNumber`, 1 on the first turn.
    pub turn_number: u32,
    /// `PlayersTakingExtraTurn.Count > 0` (this is the only player): set at the player-to-enemy
    /// switch, so it holds through the extra player turn itself.
    pub extra_turn: bool,
    /// The phase of the player's turn.
    pub phase: TurnPhase,
    /// In acquisition order.
    pub relics: Vec<Relic>,
    /// `PotionSlots`: one entry per belt slot, `None` when empty. Using a potion empties its
    /// slot; the other potions keep their slots (`Player.RemovePotionInternal`).
    pub potions: Vec<Option<Potion>>,
    /// The belt slots held by potions the engine does not port (`ModelId.Entry`), parallel to
    /// `potions`: they never act, but their slots are not empty.
    pub unported_potions: Vec<Option<&'static str>>,
    /// Index 0 is the top of the pile.
    pub draw: Pile,
    /// The hand, in order.
    pub hand: Pile,
    /// The discard pile.
    pub discard: Pile,
    /// The exhaust pile.
    pub exhaust: Pile,
    /// The Play pile.
    pub play: Pile,
    /// `PlayerCombatState._pets`; a dead Osty stays here.
    pub pets: Vec<CreatureId>,
    /// `OrbQueue.Capacity`: `Player.BaseOrbSlotCount` at combat start
    /// ([`Character::base_orb_slots`]).
    pub orb_slots: u8,
    /// `OrbQueue.Orbs`; index 0 is the front (evoked first).
    pub orbs: Vec<Orb>,
    /// `PlayerCombatState.Stars`: 0 at combat start, never negative, no cap.
    pub stars: i32,
    /// `PlayersReadyToEndTurn` contains the player: a card ended the turn (`PlayerCmd.EndTurn`),
    /// which happens once the running action finishes. Cleared when the next turn starts.
    pub ready_to_end_turn: bool,
    /// `CombatManager` card/potion effect depth for this player.
    pub effect_depth: u32,
    /// `Player.IsActiveForHooks`: cleared when the player dies (`DeactivateHooks`), so its
    /// relics, potions and cards stop listening; set again on revival. Change it with
    /// [`CombatState::set_hooks_active`].
    pub(crate) hooks_active: bool,
    /// The hooks of the unmelted relics and the potions ([`super::listeners`]), kept in step
    /// by [`CombatState::set_potion`] (relics do not change in combat).
    pub(crate) item_hooks: HookSet,
}

impl_clone_by_fields!(Player {
    character,
    creature,
    energy,
    max_energy,
    gold,
    turn_number,
    extra_turn,
    phase,
    relics: copy,
    potions: copy,
    unported_potions: copy,
    draw,
    hand,
    discard,
    exhaust,
    play,
    pets: copy,
    orb_slots,
    orbs: copy,
    stars,
    ready_to_end_turn,
    effect_depth,
    hooks_active,
    item_hooks,
});

impl Player {
    /// The cards of `pile`, in pile order.
    #[must_use]
    pub fn pile(&self, pile: PileType) -> &[CardId] {
        match pile {
            PileType::Hand => &self.hand,
            PileType::Draw => &self.draw,
            PileType::Discard => &self.discard,
            PileType::Exhaust => &self.exhaust,
            PileType::Play => &self.play,
        }
    }

    /// The cards of `pile`, to reorder them. Adding or removing cards goes through the
    /// [`CombatState`] pile methods, which keep [`CombatState::pile_of`] in step.
    pub(crate) fn pile_mut(&mut self, pile: PileType) -> &mut Vec<CardId> {
        match pile {
            PileType::Hand => &mut self.hand,
            PileType::Draw => &mut self.draw,
            PileType::Discard => &mut self.discard,
            PileType::Exhaust => &mut self.exhaust,
            PileType::Play => &mut self.play,
        }
    }

    /// `Potions`: the potions in the belt, in slot order.
    pub fn potions(&self) -> impl Iterator<Item = Potion> + '_ {
        self.potions.iter().flatten().copied()
    }

    /// The hooks of the unmelted relics and the potions, counted ([`Player::item_hooks`]).
    pub(crate) fn count_item_hooks(&self) -> HookSet {
        let relics = self
            .relics
            .iter()
            .filter(|r| !r.melted)
            .fold(HookSet::EMPTY, |hooks, r| hooks.union(r.kind.hooks()));
        self.potions()
            .fold(relics, |hooks, p| hooks.union(p.kind.hooks()))
    }

    /// The first belt slot that holds no potion, ported or not (`Player.AddPotionInternal`).
    #[must_use]
    pub fn first_empty_potion_slot(&self) -> Option<usize> {
        (0..self.potions.len())
            .find(|&i| self.potions[i].is_none() && self.unported_potions[i].is_none())
    }

    /// The combat pile holding `card`, by search.
    fn find_pile(&self, card: CardId) -> Option<PileType> {
        PileType::LISTENER_ORDER
            .into_iter()
            .find(|&p| self.pile(p).contains(&card))
    }
}

/// [`CombatState::prompting`]. Two states that hold the same models are equal whatever count
/// they carry: unit tests edit the state without keeping it in step.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PromptCount(u32);

impl PartialEq for PromptCount {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for PromptCount {}

impl std::hash::Hash for PromptCount {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

/// Everything about one combat. Plain data: clone it to snapshot.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CombatState {
    /// The run's RNG streams; they persist across combats.
    pub rng: RunRngSet,
    /// The ascension level.
    pub ascension: u8,
    /// `CombatState.Encounter`.
    pub encounter: Encounter,
    /// The run deck as of combat start, in deck order: the cards' `DeckVersion`s, whose
    /// counters a combat card can change (`TheScythe`). The run reads them back.
    pub deck: Vec<DeckCard>,
    /// `RunState.CurrentRoom.RoomType`: the encounter's room type (`CombatRoom.RoomType`),
    /// `Monster`, `Elite` or `Boss`.
    pub room_type: RoomType,
    /// `RunState.CurrentActIndex`.
    pub act_index: i32,
    /// `RunState.CurrentMapCoord` as `(col, row)`.
    pub map_coord: Option<(i32, i32)>,
    /// `RoundNumber`, 1 on the first round.
    pub round: u32,
    /// Whose turn it is.
    pub side: Side,
    /// Every creature ever created, indexed by [`CreatureId`]; removed ones stay here.
    pub creatures: Vec<Creature>,
    /// `Allies`: the player creature, then pets in insertion order.
    pub allies: Vec<CreatureId>,
    /// `Enemies`, in slot order. Change it with [`CombatState::push_enemy`] and
    /// [`CombatState::remove_enemy`], which keep [`CombatState::living_primary_enemies`] in step.
    pub enemies: Vec<CreatureId>,
    /// How many of [`CombatState::enemies`] are living primary enemies: `is_ending` asks this
    /// many times per action. Every change to an enemy's HP, its Minion power or the enemy list
    /// recounts it ([`CombatState::recount_enemies`]); debug builds compare it with the scan on
    /// every use.
    pub(crate) living_primary_enemies: u32,
    /// How many models in combat may prompt, counted as [`CombatState::may_prompt`] would scan
    /// them, so that a step asks one number instead of scanning every card, power, relic and
    /// potion. Every change to those goes through a method that keeps it in step
    /// ([`CombatState::push_card`], [`CombatState::set_affliction`],
    /// [`CombatState::set_enchantment`], [`CombatState::set_potion`],
    /// [`CombatState::push_creature`], [`CombatState::push_power`] and the power removals);
    /// debug builds compare it with the scan on every use. Derived data, so it is left out of
    /// equality and hashing ([`PromptCount`]).
    pub(crate) prompting: PromptCount,
    /// The hook unions of the cards and of everything a walk reaches ([`super::listeners`]).
    /// Derived data, left out of equality and hashing.
    pub(crate) hook_index: HookIndex,
    /// The player.
    pub player: Player,
    /// Every combat card, indexed by [`CardId`].
    pub cards: Vec<Card>,
    /// The cards' energy cost modifiers and temporary star costs.
    pub(crate) cost_lists: CostLists,
    /// The powers' instance data; read it with [`CombatState::power_data`].
    pub(crate) power_data: PowerDataTable,
    /// `CombatManager.IsInProgress`.
    pub in_progress: bool,
    /// The run's modifiers that act inside this fight.
    pub(crate) modifiers: crate::run::modifiers::CombatModifiers,
    /// `CombatManager.IsStarting`: from set-up until combat is in progress; guarded hooks
    /// still reach their listeners meanwhile (`Hook.cs:55`).
    pub(crate) starting: bool,
    /// `PendingLoss != null`.
    pub pending_loss: bool,
    /// The result, once the combat is over.
    pub result: Option<CombatResult>,
    pub(crate) next_power_id: u32,
    /// The next [`PotionId`].
    pub(crate) next_potion_id: u32,
    /// The next [`AttackId`](super::commands::damage::AttackId).
    pub(crate) next_attack_id: u32,
    /// The next [`OrbId`].
    pub(crate) next_orb_id: u32,
    /// `CombatRoom.ExtraRewards` for the player: rewards an `AfterCombatEnd` listener added
    /// (`AddExtraReward`); the run offers them after the room's own.
    pub extra_rewards: Vec<ExtraReward>,
    /// Deck cards `CardPileCmd.RemoveFromDeck` took out of the run deck during the fight
    /// (`SwipePower.Steal`), as indices into [`CombatState::deck`] in removal order; the run
    /// removes them when the fight ends.
    pub removed_from_deck: Vec<usize>,
    /// `EscapedCreatures`: creatures `CreatureCmd.Escape` removed, in escape order.
    pub escaped: Vec<CreatureId>,
    /// `Encounter.SpawnedEnemies`: the distinct monster models (`MonsterKind::id`) created on the
    /// enemy side this combat, at start or summoned, in first-spawn order.
    pub spawned_enemies: Vec<&'static str>,
    /// The encounter instance's own fields (`GremlinMercNormal.GoldWasStolen`).
    pub encounter_data: EncounterData,
    /// `CombatRoom.GoldProportion`: set from `CalculateGoldProportion` when the fight is won.
    pub gold_proportion: GoldProportion,
    /// `CombatHistory` entries, oldest first.
    pub(crate) history: CombatHistory,
}

impl_clone_by_fields!(CombatState {
    rng,
    ascension,
    encounter,
    deck: copy,
    room_type,
    act_index,
    map_coord,
    round,
    side,
    creatures,
    allies: copy,
    enemies: copy,
    living_primary_enemies,
    prompting,
    hook_index,
    player,
    cards: copy,
    cost_lists,
    power_data,
    in_progress,
    modifiers,
    starting,
    pending_loss,
    result,
    next_power_id,
    next_potion_id,
    next_attack_id,
    next_orb_id,
    extra_rewards: copy,
    removed_from_deck: copy,
    escaped: copy,
    spawned_enemies: copy,
    encounter_data,
    gold_proportion,
    history,
} checked);

impl CombatState {
    /// Whether any model in combat can raise a player choice (`may_prompt`): every card of
    /// the combat (in a pile or not) with its affliction and enchantment, the belt's potions,
    /// the relics, every creature's powers and the monsters. A step is snapshotted for replay
    /// only when this holds.
    #[must_use]
    pub fn may_prompt(&self) -> bool {
        debug_assert_eq!(
            self.prompting.0,
            self.count_prompting(),
            "the count of models that may prompt is out of step"
        );
        self.prompting.0 > 0
    }

    /// The models in combat that may prompt, counted ([`CombatState::prompting`]).
    fn count_prompting(&self) -> u32 {
        let cards: u32 = self.cards.iter().map(Card::prompting).sum();
        let creatures: u32 = self.creatures.iter().map(Creature::prompting).sum();
        let potions = self
            .player
            .potions()
            .filter(|p| p.kind.may_prompt())
            .count();
        let relics = self
            .player
            .relics
            .iter()
            .filter(|r| r.kind.may_prompt())
            .count();
        let items = u32::try_from(potions + relics).expect("belt and relics fit u32");
        cards + creatures + items
    }

    /// Recounts [`CombatState::prompting`]: once the state is built, and after a unit test
    /// changes it directly (`Combat::edit`).
    pub(crate) fn recount_prompting(&mut self) {
        self.prompting.0 = self.count_prompting();
    }

    /// Adds `card` to [`CombatState::cards`] (its id is its place there).
    pub(crate) fn push_card(&mut self, card: Card) {
        self.prompting.0 += card.prompting();
        self.cards.push(card);
    }

    /// Sets `card`'s affliction.
    pub(crate) fn set_affliction(&mut self, card: CardId, affliction: Option<Affliction>) {
        let piled = self.card(card).pile.is_some();
        if piled {
            self.count_card(card, false);
        }
        let c = &mut self.cards[card.index()];
        let before = c.prompting();
        c.affliction = affliction;
        self.prompting.0 = self.prompting.0 - before + c.prompting();
        if piled {
            self.count_card(card, true);
        }
    }

    /// Sets `card`'s enchantment; the card.
    pub(crate) fn set_enchantment(
        &mut self,
        card: CardId,
        enchantment: Option<Enchantment>,
    ) -> &mut Card {
        let piled = self.card(card).pile.is_some();
        if piled {
            self.count_card(card, false);
        }
        let c = &mut self.cards[card.index()];
        let before = c.prompting();
        c.enchantment = enchantment;
        self.prompting.0 = self.prompting.0 - before + c.prompting();
        if piled {
            self.count_card(card, true);
        }
        self.card_mut(card)
    }

    /// Puts `potion` in belt slot `slot` (or empties it).
    pub(crate) fn set_potion(&mut self, slot: usize, potion: Option<Potion>) {
        let prompts = |p: Option<Potion>| u32::from(p.is_some_and(|p| p.kind.may_prompt()));
        let before = prompts(self.player.potions[slot]);
        self.player.potions[slot] = potion;
        self.prompting.0 = self.prompting.0 - before + prompts(potion);
        let hooks = self.player.count_item_hooks();
        if hooks != self.player.item_hooks {
            self.player.item_hooks = hooks;
            self.refresh_hooks();
        }
    }

    /// Adds `creature` to [`CombatState::creatures`] (its id is its place there), not yet in
    /// combat.
    pub(crate) fn push_creature(&mut self, mut creature: Creature) {
        self.prompting.0 += creature.prompting();
        creature.monster_hooks = creature.count_monster_hooks();
        creature.hooks = creature.count_hooks();
        self.creatures.push(creature);
    }

    /// Runs `change` on creature `id`, then brings the union a walk reaches in step if the
    /// creature's hooks changed.
    fn change_creature<R>(&mut self, id: CreatureId, change: impl FnOnce(&mut Creature) -> R) -> R {
        let creature = self.creature_mut(id);
        let before = creature.hooks;
        let result = change(creature);
        if self.creature(id).hooks != before {
            self.refresh_hooks();
        }
        result
    }

    /// Adds `power` after creature `id`'s other powers ([`Creature::push_power`]).
    pub(crate) fn push_power(&mut self, id: CreatureId, power: Power) {
        self.prompting.0 += u32::from(power.kind.may_prompt());
        self.change_creature(id, |c| c.push_power(power));
    }

    /// Takes creature `id`'s power at `index` off it ([`Creature::take_power`]).
    pub(crate) fn take_power(&mut self, id: CreatureId, index: usize) -> Power {
        let power = self.change_creature(id, |c| c.take_power(index));
        self.prompting.0 -= u32::from(power.kind.may_prompt());
        power
    }

    /// Takes creature `id`'s powers for which `take` holds off it
    /// ([`Creature::take_powers_where`]).
    pub(crate) fn take_powers_where(
        &mut self,
        id: CreatureId,
        take: impl Fn(&Power) -> bool,
    ) -> Vec<Power> {
        let taken = self.change_creature(id, |c| c.take_powers_where(take));
        self.forget_prompting_powers(&taken);
        taken
    }

    /// Every power leaves creature `id` ([`Creature::remove_all_powers`]).
    pub(crate) fn remove_all_powers(&mut self, id: CreatureId) {
        let powers = &self.creature(id).powers;
        let prompting = powers.iter().filter(|p| p.kind.may_prompt()).count();
        self.prompting.0 -= u32::try_from(prompting).expect("power count fits u32");
        self.change_creature(id, Creature::remove_all_powers);
    }

    /// Sets `Player.IsActiveForHooks` ([`Player::hooks_active`]).
    pub(crate) fn set_hooks_active(&mut self, active: bool) {
        self.player.hooks_active = active;
        self.refresh_hooks();
    }

    fn forget_prompting_powers(&mut self, powers: &[Power]) {
        let count = powers.iter().filter(|p| p.kind.may_prompt()).count();
        self.prompting.0 -= u32::try_from(count).expect("power count fits u32");
    }

    /// The creature with `id`.
    ///
    /// # Panics
    /// If this fight has no creature `id` (an id from another fight, or made up).
    #[must_use]
    pub fn creature(&self, id: CreatureId) -> &Creature {
        &self.creatures[id.index()]
    }

    pub(crate) fn creature_mut(&mut self, id: CreatureId) -> &mut Creature {
        &mut self.creatures[id.index()]
    }

    /// The relic with `id`.
    ///
    /// # Panics
    /// If this fight has no relic `id` (an id from another fight, or made up).
    #[must_use]
    pub fn relic(&self, id: RelicId) -> &Relic {
        &self.player.relics[id.index()]
    }

    /// The relic with `id`, mutably (its [`RelicData`]).
    pub(crate) fn relic_mut(&mut self, id: RelicId) -> &mut Relic {
        &mut self.player.relics[id.index()]
    }

    /// The card with `id`.
    ///
    /// # Panics
    /// If this fight has no card `id` (an id from another fight, or made up).
    #[must_use]
    pub fn card(&self, id: CardId) -> &Card {
        &self.cards[id.index()]
    }

    pub(crate) fn card_mut(&mut self, id: CardId) -> &mut Card {
        &mut self.cards[id.index()]
    }

    /// The combat pile holding `card`, if any.
    #[must_use]
    pub fn pile_of(&self, card: CardId) -> Option<PileType> {
        let pile = self.card(card).pile;
        debug_assert_eq!(pile, self.player.find_pile(card), "card pile out of step");
        pile
    }

    /// Takes `card` out of its pile, if any, and returns that pile.
    #[inline]
    pub(crate) fn remove_from_pile(&mut self, card: CardId) -> Option<PileType> {
        let pile = self.pile_of(card)?;
        let cards = self.player.pile_mut(pile);
        let at = cards
            .iter()
            .position(|&c| c == card)
            .expect("a card is in the pile it names");
        // `Vec::remove` without its `memmove` call: piles are short.
        for i in at + 1..cards.len() {
            cards[i - 1] = cards[i];
        }
        cards.pop();
        debug_assert!(!cards.contains(&card), "a card is in its pile once");
        self.count_card(card, false);
        self.card_mut(card).pile = None;
        Some(pile)
    }

    /// Moves `card` into `pile` at the place `index` picks once the card has left its old pile
    /// (`None`: the bottom); returns the old pile. The same as [`CombatState::remove_from_pile`]
    /// then [`CombatState::insert_into_pile`], without counting a card that stays in the piles
    /// out of the hook unions and back in.
    #[inline]
    pub(crate) fn move_card(
        &mut self,
        card: CardId,
        pile: PileType,
        index: impl FnOnce(&mut Self) -> Option<usize>,
    ) -> Option<PileType> {
        let old = self.pile_of(card);
        if let Some(old) = old {
            let cards = self.player.pile_mut(old);
            let at = cards
                .iter()
                .position(|&c| c == card)
                .expect("a card is in the pile it names");
            // `Vec::remove` without its `memmove` call: piles are short.
            for i in at + 1..cards.len() {
                cards[i - 1] = cards[i];
            }
            cards.pop();
            debug_assert!(!cards.contains(&card), "a card is in its pile once");
            self.card_mut(card).pile = None;
        }
        let at = index(self);
        let cards = self.player.pile_mut(pile);
        cards.insert(at.unwrap_or(cards.len()), card);
        self.card_mut(card).pile = Some(pile);
        if old.is_none() {
            self.count_card(card, true);
        }
        old
    }

    /// Moves `cards` to the bottom of `pile`, in order, with the same result as taking each in
    /// turn out of its pile ([`CombatState::remove_from_pile`]) and putting it at the bottom of
    /// `pile`: the piles keep their other cards in order, and `pile` ends with `cards`. Appends
    /// each card with its old pile to `moved` (built in place: a returned list is copied
    /// through `memcpy`) and returns true, or returns false without moving anything if a card
    /// is in no pile, is listed twice, or has an id past the bitset (the caller then moves them
    /// one by one).
    pub(crate) fn move_to_pile_bottom(
        &mut self,
        cards: &[CardId],
        pile: PileType,
        moved: &mut MovedCards,
    ) -> bool {
        let mut seen = [0_u64; 4];
        // Cards moved out of each pile, by `PileType as usize`.
        let mut leaving = [0_usize; PileType::LISTENER_ORDER.len()];
        for &card in cards {
            let i = card.index();
            if i >= 256 || seen[i / 64] & (1 << (i % 64)) != 0 {
                return false;
            }
            let Some(from) = self.pile_of(card) else {
                return false;
            };
            seen[i / 64] |= 1 << (i % 64);
            leaving[from.index()] += 1;
        }
        // Nothing has changed yet: from here on the batch moves.
        moved.extend(cards.iter().map(|&id| {
            let card = self.card_mut(id);
            let old = card.pile;
            card.pile = Some(pile);
            (id, old)
        }));
        for from in PileType::LISTENER_ORDER {
            let count = leaving[from.index()];
            if count == 0 {
                continue;
            }
            let from = self.player.pile_mut(from);
            // The cards are distinct and each is in the pile it names, so a pile losing as many
            // cards as it holds loses all of them.
            if count == from.len() {
                debug_assert!(
                    from.iter()
                        .all(|c| seen[c.index() / 64] & (1 << (c.0 % 64)) != 0),
                    "every card of a pile losing all its cards moves"
                );
                from.clear();
            } else {
                from.retain(|c| {
                    let i = c.index();
                    i >= 256 || seen[i / 64] & (1 << (i % 64)) == 0
                });
            }
        }
        self.player.pile_mut(pile).extend_from_slice(cards);
        true
    }

    /// Moves `card`, the draw pile's top card, to the bottom of the hand: the result of
    /// [`CombatState::remove_from_pile`] and [`CombatState::insert_into_pile`] without the
    /// search for the card's pile and place.
    #[inline]
    pub(crate) fn draw_top_to_hand(&mut self, card: CardId) {
        let draw = &mut self.player.draw;
        debug_assert_eq!(draw.first(), Some(&card), "the card is the draw pile's top");
        for i in 1..draw.len() {
            draw[i - 1] = draw[i];
        }
        draw.pop();
        self.player.hand.push(card);
        self.card_mut(card).pile = Some(PileType::Hand);
    }

    /// Puts `card`, which is in no pile, into `pile` at `index`, or at the bottom if `None`.
    #[inline]
    pub(crate) fn insert_into_pile(&mut self, card: CardId, pile: PileType, index: Option<usize>) {
        debug_assert_eq!(self.pile_of(card), None, "the card is already in a pile");
        let cards = self.player.pile_mut(pile);
        cards.insert(index.unwrap_or(cards.len()), card);
        self.card_mut(card).pile = Some(pile);
        self.count_card(card, true);
    }

    /// Empties every combat pile.
    pub(crate) fn clear_piles(&mut self) {
        for pile in PileType::LISTENER_ORDER {
            // Taken and put back empty, so the pile keeps its buffer.
            let mut cards = std::mem::take(self.player.pile_mut(pile));
            for &card in &cards {
                self.card_mut(card).pile = None;
            }
            cards.clear();
            *self.player.pile_mut(pile) = cards;
        }
        self.forget_card_listeners();
    }

    /// `HittableEnemies`: living enemies no listener's `ShouldAllowHitting` vetoes.
    #[must_use]
    pub fn hittable_enemies(&self) -> Creatures {
        self.hooked().hittable_enemies()
    }

    /// `ContainsCreature`: in `Allies` or `Enemies`.
    #[must_use]
    pub fn contains_creature(&self, id: CreatureId) -> bool {
        self.allies.contains(&id) || self.enemies.contains(&id)
    }

    /// `PlayerCreatures`: allies that are players (never pets).
    #[must_use]
    #[inline]
    pub fn player_creatures(&self) -> Creatures {
        // The only player is `player.creature`; every other ally is a pet.
        let me = self.player.creature;
        let mut players = Creatures::new();
        if self.allies.contains(&me) {
            players.push(me);
        }
        debug_assert!(
            players.iter().copied().eq(self
                .allies
                .iter()
                .copied()
                .filter(|&c| self.creature(c).is_player())),
            "the player creature is the only player ally"
        );
        players
    }

    /// Creatures on `side` (`Allies` or `Enemies`).
    #[must_use]
    #[inline]
    pub fn creatures_on(&self, side: Side) -> Creatures {
        let creatures = match side {
            Side::Player => &self.allies,
            Side::Enemy => &self.enemies,
        };
        Creatures::from_slice(creatures)
    }

    /// `Player.Osty`: the player's Osty pet, alive or dead.
    #[must_use]
    pub fn osty(&self) -> Option<CreatureId> {
        self.player.pets.iter().copied().find(|&c| {
            self.creature(c)
                .monster
                .as_ref()
                .is_some_and(Monster::is_osty)
        })
    }

    /// `Player.IsOstyAlive`.
    #[must_use]
    pub fn is_osty_alive(&self) -> bool {
        self.osty().is_some_and(|c| self.creature(c).is_alive())
    }

    /// `CombatManager.IsEnding` (`IsCombatEnding`, `CombatManager.cs:417-435`): in progress,
    /// and a loss is pending, or no living primary enemy is left and no listener's
    /// `ShouldStopCombatFromEnding` keeps combat going.
    #[must_use]
    pub fn is_ending(&self) -> bool {
        self.is_ending_unless(|| self.should_stop_combat_from_ending())
    }

    /// [`CombatState::is_ending`] with `stop` as `ShouldStopCombatFromEnding` (only asked when
    /// no living primary enemy is left).
    #[inline]
    pub(crate) fn is_ending_unless(&self, stop: impl FnOnce() -> bool) -> bool {
        debug_assert_eq!(
            self.living_primary_enemies,
            self.count_living_primary_enemies(),
            "the living primary enemy count is out of step"
        );
        self.in_progress && (self.pending_loss || (self.living_primary_enemies == 0 && !stop()))
    }

    /// The living primary enemies among [`CombatState::enemies`], counted.
    fn count_living_primary_enemies(&self) -> u32 {
        let count = self
            .enemies
            .iter()
            .filter(|&&e| {
                let c = self.creature(e);
                c.is_alive() && c.is_primary_enemy()
            })
            .count();
        u32::try_from(count).expect("enemy count fits u32")
    }

    /// Recounts [`CombatState::living_primary_enemies`]: after any change that can make an
    /// enemy die, revive, or gain or lose Minion.
    #[inline(never)]
    pub(crate) fn recount_enemies(&mut self) {
        self.living_primary_enemies = self.count_living_primary_enemies();
    }

    /// Adds `id` to [`CombatState::enemies`] (after the others).
    pub(crate) fn push_enemy(&mut self, id: CreatureId) {
        self.enemies.push(id);
        self.recount_enemies();
        self.refresh_hooks();
    }

    /// Takes `id` out of [`CombatState::enemies`].
    pub(crate) fn remove_enemy(&mut self, id: CreatureId) {
        self.enemies.retain(|&e| e != id);
        self.recount_enemies();
        self.refresh_hooks();
    }

    /// Adds `id` to [`CombatState::allies`] (after the others).
    pub(crate) fn push_ally(&mut self, id: CreatureId) {
        self.allies.push(id);
        self.refresh_hooks();
    }

    /// Takes `id` out of [`CombatState::allies`].
    pub(crate) fn remove_ally(&mut self, id: CreatureId) {
        self.allies.retain(|&a| a != id);
        self.refresh_hooks();
    }

    /// `SortEnemiesBySlotName` (`CombatState.cs:496-502`): `List.Sort` (.NET's introsort,
    /// [`introsort_by`](crate::shuffle::introsort_by)) by the index of each enemy's slot in the
    /// encounter's `Slots` (-1, first, for none).
    pub(crate) fn sort_enemies_by_slot_name(&mut self) {
        let slots = self.encounter.slots();
        let mut enemies = std::mem::take(&mut self.enemies);
        let index = |id: CreatureId| {
            self.creature(id)
                .slot
                .and_then(|name| slots.iter().position(|&slot| slot == name))
        };
        crate::shuffle::introsort_by(&mut enemies, |&a, &b| index(a).cmp(&index(b)));
        self.enemies = enemies;
    }

    /// `CombatManager.IsOverOrEnding`.
    #[must_use]
    pub fn is_over_or_ending(&self) -> bool {
        !self.in_progress || self.is_ending()
    }
}
