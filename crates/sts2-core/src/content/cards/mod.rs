//! Cards: one file per card, each a unit struct implementing [`CardModel`] (static data and
//! `OnPlay`) and [`Hooks`](crate::combat::hooks::Hooks).
//!
//! To add a card: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates the
//! variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Registration").

include!(concat!(env!("OUT_DIR"), "/content/cards.rs"));

pub mod pools;
pub mod vars;

pub use vars::{CardVar, Multiplier, PreviewMode, Var, VarClass};

use std::cmp::Ordering;

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, ResultLocation};
use crate::decimal::Decimal;

/// A card model's own per-instance fields (C# fields of the `CardModel` subclass).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CardData {
    /// No per-card fields.
    #[default]
    None,
    /// `TheScythe`'s fields ([`the_scythe::Data`]).
    TheScythe(the_scythe::Data),
    /// `Regret._cardsInHand`.
    Regret(i32),
    /// `UpMySleeve._timesPlayedThisCombat`.
    UpMySleeve(i32),
    /// `Bolas`'s fields ([`bolas::Data`]).
    Bolas(bolas::Data),
    /// `SovereignBlade`'s fields ([`sovereign_blade::Data`]).
    SovereignBlade(sovereign_blade::Data),
    /// `Rampage`'s fields ([`rampage::Data`]).
    Rampage(rampage::Data),
    /// `Thrash`'s fields ([`thrash::Data`]).
    Thrash(thrash::Data),
    /// `KinglyPunch`'s fields ([`kingly_punch::Data`]).
    KinglyPunch(kingly_punch::Data),
    /// `GeneticAlgorithm`'s fields ([`genetic_algorithm::Data`]).
    GeneticAlgorithm(genetic_algorithm::Data),
    /// `Claw`'s fields ([`claw::Data`]).
    Claw(claw::Data),
    /// `Maul`'s fields ([`maul::Data`]).
    Maul(maul::Data),
    /// `Guilty.CombatsSeen` or `Dowsing.RoomsEntered`, the deck card's counter; for a `SpoilsMap`,
    /// 1 once its `SpoilsCoord` is set.
    Counter(u32),
    /// `Wither`'s fields ([`wither::Data`]).
    Wither(wither::Data),
    /// `MadScience.TinkerTimeType` and `TinkerTimeRider`.
    MadScience(mad_science::Data),
}

/// The fields a combat card cloned from a deck card starts with, from the deck card's saved
/// counter ([`DeckCard::counter`](crate::combat::DeckCard)).
#[must_use]
pub fn data_from_deck(kind: CardKind, counter: u32) -> CardData {
    match kind {
        CardKind::TheScythe if counter > 0 => the_scythe::data_from_deck(counter),
        CardKind::GeneticAlgorithm if counter > 0 => genetic_algorithm::data_from_deck(counter),
        CardKind::Guilty | CardKind::Dowsing | CardKind::SpoilsMap if counter > 0 => {
            CardData::Counter(counter)
        }
        CardKind::MadScience => mad_science::data_from_deck(counter),
        _ => CardData::None,
    }
}

/// `CardType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardType {
    /// `CardType.Attack`.
    Attack,
    /// `CardType.Skill`.
    Skill,
    /// `CardType.Power`.
    Power,
    /// `CardType.Status`.
    Status,
    /// `CardType.Curse`.
    Curse,
    /// `CardType.Quest`.
    Quest,
}

/// `CardRarity` (`Entities.Cards/CardRarity.cs`), in enum order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardRarity {
    /// `CardRarity.None`.
    None,
    /// `CardRarity.Basic`.
    Basic,
    /// `CardRarity.Common`.
    Common,
    /// `CardRarity.Uncommon`.
    Uncommon,
    /// `CardRarity.Rare`.
    Rare,
    /// `CardRarity.Ancient`.
    Ancient,
    /// `CardRarity.Event`.
    Event,
    /// `CardRarity.Token`.
    Token,
    /// `CardRarity.Status`.
    Status,
    /// `CardRarity.Curse`.
    Curse,
    /// `CardRarity.Quest`.
    Quest,
}

/// `CardKeyword` (`Entities.Cards/CardKeyword.cs`), without `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardKeyword {
    /// Exhausted instead of discarded after a play.
    Exhaust,
    /// Exhausted when the turn ends with it in hand.
    Ethereal,
    /// Starts the combat on top of the draw pile.
    Innate,
    /// Cannot be played (`CanPlay`); autoplay moves it to its result pile instead.
    Unplayable,
    /// Stays in hand when the hand is flushed.
    Retain,
    /// Autoplayed when discarded (no `CardCmd.Discard` yet).
    Sly,
    /// Cannot be removed from the deck (run layer).
    Eternal,
}

impl_index!(CardKeyword);

/// A set of [`CardKeyword`]s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Keywords(u8);

impl Keywords {
    /// No keywords.
    pub const NONE: Self = Self(0);

    const fn bit(keyword: CardKeyword) -> u8 {
        1 << keyword.index()
    }

    /// The set plus `keyword`.
    #[must_use]
    pub const fn with(self, keyword: CardKeyword) -> Self {
        Self(self.0 | Self::bit(keyword))
    }

    /// Whether `keyword` is in the set.
    #[must_use]
    pub const fn contains(self, keyword: CardKeyword) -> bool {
        self.0 & Self::bit(keyword) != 0
    }

    /// `AddKeyword`.
    pub const fn insert(&mut self, keyword: CardKeyword) {
        self.0 |= Self::bit(keyword);
    }

    /// `RemoveKeyword`.
    pub const fn remove(&mut self, keyword: CardKeyword) {
        self.0 &= !Self::bit(keyword);
    }
}

/// `TargetType` (the subset in scope).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetType {
    /// `Self`: no target.
    Myself,
    /// `None`: no target.
    None,
    /// `AnyEnemy`: one living enemy.
    AnyEnemy,
    /// `AllEnemies`: no target; the card hits every opponent.
    AllEnemies,
    /// `RandomEnemy`: no target; the card picks a random opponent per hit.
    RandomEnemy,
    /// A player creature (potions; `IsPlayer`, so never a pet).
    AnyPlayer,
}

impl TargetType {
    /// Whether a manual play names a target creature.
    #[must_use]
    pub const fn needs_target(self) -> bool {
        matches!(self, Self::AnyEnemy | Self::AnyPlayer)
    }
}

/// A card's static data and `OnPlay` (`CardModel`).
#[expect(
    unused_variables,
    reason = "default bodies ignore their arguments, like the C# virtual methods"
)]
pub(crate) trait CardModel: Hooks<Me = CardId> {
    /// Whether this model can raise a player choice ([`crate::combat::DecisionRequest`]) from
    /// any of its entry points or hooks, directly or by creating, transforming into or playing a
    /// model that can. The combat snapshots a step only while such a model is in combat, so a
    /// wrong `false` panics at the choice (debug builds also check replay determinism).
    fn may_prompt(&self) -> bool {
        false
    }

    /// The English display name (`Title`, without the upgrade `+`).
    fn name(&self) -> &'static str;
    /// `CanonicalEnergyCost` (ignored for X costs).
    fn cost(&self) -> i32;
    /// `HasEnergyCostX`: the play spends all energy and captures it as X.
    fn has_energy_cost_x(&self) -> bool {
        false
    }
    /// `CanonicalStarCost`: -1 for no star cost (ignored for X star costs).
    fn star_cost(&self) -> i32 {
        -1
    }
    /// `HasStarCostX`: the play spends all stars (`LastStarsSpent` is X).
    fn has_star_cost_x(&self) -> bool {
        false
    }
    /// `CardType` of the canonical card.
    fn card_type(&self) -> CardType;
    /// `Type` of a card instance, for a card whose type is one of its own fields (`MadScience`'s
    /// `TinkerTimeType`); defaults to [`CardModel::card_type`]. Read it with [`Card::card_type`].
    fn instance_card_type(&self, card: &Card) -> CardType {
        self.card_type()
    }
    /// `Rarity`.
    fn rarity(&self) -> CardRarity;
    /// `TargetType` of the canonical card.
    fn target(&self) -> TargetType;
    /// `TargetType` of a combat card, for a card whose target type depends on the state
    /// (`SovereignBlade`); defaults to [`CardModel::target`].
    fn target_in(&self, s: &CombatState, me: CardId) -> TargetType {
        self.target()
    }
    /// `CanonicalKeywords`.
    fn keywords(&self) -> Keywords {
        Keywords::NONE
    }
    /// `GainsBlock` of the canonical card.
    fn gains_block(&self) -> bool {
        false
    }
    /// `GainsBlock` of a card instance (`MadScience`); defaults to [`CardModel::gains_block`].
    /// Read it with [`Card::gains_block`].
    fn instance_gains_block(&self, card: &Card) -> bool {
        self.gains_block()
    }
    /// `CanonicalTags` contains `CardTag.OstyAttack`.
    fn is_osty_attack(&self) -> bool {
        false
    }
    /// `CanonicalTags` contains `CardTag.Strike`.
    fn is_strike(&self) -> bool {
        false
    }
    /// `CanonicalTags` contains `CardTag.Shiv`.
    fn is_shiv(&self) -> bool {
        false
    }
    /// `CanonicalTags` contains `CardTag.Defend`.
    fn is_defend(&self) -> bool {
        false
    }
    /// `CanonicalTags` contains `CardTag.Minion`.
    fn is_minion(&self) -> bool {
        false
    }
    /// `CanonicalVars` with their `OnUpgrade` steps, in order (see [`vars`]). `OnPlay` reads
    /// them back with [`Card::var`] and [`CombatState::calculate`].
    fn vars(&self) -> &'static [CardVar] {
        &[]
    }
    /// The instance's `BaseValue` of var `name`, given `value`, its canonical one at the card's
    /// upgrade level: for a var the card class changes (`Rampage`'s `Damage`).
    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        value
    }
    /// `MaxUpgradeLevel`.
    fn max_upgrade_level(&self) -> u8 {
        1
    }
    /// The instance-state part of `OnUpgrade`: `AddKeyword`, `RemoveKeyword`,
    /// `EnergyCost.UpgradeBy`. Upgraded `DynamicVars` come from [`CardModel::vars`].
    fn on_upgrade(&self, card: &mut Card) {}
    /// `IsPlayable`: card logic that blocks a manual play (`CanPlay`).
    fn is_playable(&self, s: &CombatState, me: CardId) -> bool {
        true
    }
    /// `ShouldGlowGoldInternal`: the card's condition for its gold glow in the hand (a
    /// bonus is on, `Dismantle` against a Vulnerable enemy). Display only.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        false
    }
    /// `GetResultLocationForCardPlay`, given `base.GetResultLocationForCardPlay()` (the engine
    /// computes the base location: None for powers, Exhaust, else Discard at the bottom).
    fn result_location_for_card_play(&self, base: ResultLocation) -> ResultLocation {
        base
    }
    /// `OnPlay(choiceContext, cardPlay)`.
    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow;
    /// `HasTurnEndInHandEffect`: run [`CardModel::on_turn_end_in_hand`] when the turn ends with
    /// the card in hand (`CombatManager.DoTurnEnd`).
    fn has_turn_end_in_hand_effect(&self) -> bool {
        false
    }
    /// `OnTurnEndInHand(choiceContext)`; the card is in the Play pile meanwhile.
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        Ok(())
    }
    /// `AfterCreated` (sync): right after `CombatState.CreateCard`, before the card has a pile.
    fn after_created(&self, s: &mut CombatState, me: CardId) {}
    /// `AfterDowngraded` (sync), at the end of `DowngradeInternal`.
    fn after_downgraded(&self, s: &mut CombatState, me: CardId) {}
}

impl CardKind {
    /// `ModelId.Entry`, the slugified class name ([`Self::id`]); used by `CardModel.CompareTo`.
    #[must_use]
    pub const fn entry(self) -> &'static str {
        self.id()
    }

    /// The English display name (`Title`, without the upgrade `+`).
    #[must_use]
    pub fn name(self) -> &'static str {
        with_model!(self, m => m.name())
    }

    /// Whether the model can raise a player choice (see `may_prompt` in the content API).
    #[must_use]
    pub fn may_prompt(self) -> bool {
        with_model!(self, m => m.may_prompt())
    }

    /// The printed energy cost (`CanonicalEnergyCost`; 0 for X costs).
    #[must_use]
    pub fn cost(self) -> i32 {
        if self.has_energy_cost_x() {
            0
        } else {
            with_model!(self, m => m.cost())
        }
    }

    /// `HasEnergyCostX`.
    #[must_use]
    pub fn has_energy_cost_x(self) -> bool {
        with_model!(self, m => m.has_energy_cost_x())
    }

    /// `CanonicalStarCost` (-1 for none).
    #[must_use]
    pub fn star_cost(self) -> i32 {
        with_model!(self, m => m.star_cost())
    }

    /// `HasStarCostX`.
    #[must_use]
    pub fn has_star_cost_x(self) -> bool {
        with_model!(self, m => m.has_star_cost_x())
    }

    /// `CanonicalKeywords`.
    #[must_use]
    pub fn keywords(self) -> Keywords {
        with_model!(self, m => m.keywords())
    }

    /// `CardType` of the canonical card (a card instance: [`Card::card_type`]).
    #[must_use]
    pub fn card_type(self) -> CardType {
        with_model!(self, m => m.card_type())
    }

    /// `Rarity`.
    #[must_use]
    pub fn rarity(self) -> CardRarity {
        with_model!(self, m => m.rarity())
    }

    /// `GainsBlock` of the canonical card (a card instance: [`Card::gains_block`]).
    #[must_use]
    pub fn gains_block(self) -> bool {
        with_model!(self, m => m.gains_block())
    }

    /// `CanonicalTags` contains `CardTag.OstyAttack`.
    #[must_use]
    pub fn is_osty_attack(self) -> bool {
        with_model!(self, m => m.is_osty_attack())
    }

    /// `CanonicalTags` contains `CardTag.Strike`.
    #[must_use]
    pub fn is_strike(self) -> bool {
        with_model!(self, m => m.is_strike())
    }

    /// `CanonicalTags` contains `CardTag.Shiv`.
    #[must_use]
    pub fn is_shiv(self) -> bool {
        with_model!(self, m => m.is_shiv())
    }

    /// `CanonicalTags` contains `CardTag.Defend`.
    #[must_use]
    pub fn is_defend(self) -> bool {
        with_model!(self, m => m.is_defend())
    }

    /// `CanonicalTags` contains `CardTag.Minion`.
    #[must_use]
    pub fn is_minion(self) -> bool {
        with_model!(self, m => m.is_minion())
    }

    /// `VisualCardPool.IsColorless` ([`pools::is_visual_pool_colorless`]).
    #[must_use]
    pub fn is_colorless(self) -> bool {
        pools::is_visual_pool_colorless(self.entry())
    }

    /// `TargetType` of the canonical card (a combat card: [`CombatState::target_type`]).
    #[must_use]
    pub fn target(self) -> TargetType {
        with_model!(self, m => m.target())
    }

    /// `CardModel.IsUpgradable` for a card at `upgrade_level`.
    #[must_use]
    pub fn is_upgradable(self, upgrade_level: u8) -> bool {
        upgrade_level < with_model!(self, m => m.max_upgrade_level())
    }

    /// Whether the card's canonical keywords include `Exhaust` (an instance may differ:
    /// [`Card::keywords`]).
    #[must_use]
    pub fn exhausts(self) -> bool {
        self.keywords().contains(CardKeyword::Exhaust)
    }

    /// Whether the card's canonical keywords include `Unplayable`.
    #[must_use]
    pub fn is_unplayable(self) -> bool {
        self.keywords().contains(CardKeyword::Unplayable)
    }

    /// Whether the card's canonical keywords include `Ethereal`.
    #[must_use]
    pub fn is_ethereal(self) -> bool {
        self.keywords().contains(CardKeyword::Ethereal)
    }

    /// `HasTurnEndInHandEffect`.
    #[must_use]
    pub fn has_turn_end_in_hand_effect(self) -> bool {
        with_model!(self, m => m.has_turn_end_in_hand_effect())
    }
}

/// `CardModel.CompareTo`: model id (category, then entry, ordinal), then upgrade level
/// (`CardModel.cs:2272-2294`, `ModelId.cs:42`). Every card shares the category. The entries
/// are compared by their rank in ordinal order ([`CardKind::id_rank`]).
pub(crate) fn compare(a: &Card, b: &Card) -> Ordering {
    a.kind
        .id_rank()
        .cmp(&b.kind.id_rank())
        .then(a.upgrade_level.cmp(&b.upgrade_level))
}

/// A key that orders cards as [`compare`] does: the id's rank, then the upgrade level.
pub(crate) fn sort_key(card: &Card) -> u32 {
    u32::from(card.kind.id_rank()) << 8 | u32::from(card.upgrade_level)
}

/// `CardModel.TargetType` of a combat card ([`CardModel::target_in`]).
pub(crate) fn target_type(s: &CombatState, card: CardId) -> TargetType {
    with_model!(s.card(card).kind, m => m.target_in(s, card))
}

/// `Type` of a card instance ([`CardModel::instance_card_type`]).
pub(crate) fn instance_card_type(card: &Card) -> CardType {
    with_model!(card.kind, m => m.instance_card_type(card))
}

/// `GainsBlock` of a card instance ([`CardModel::instance_gains_block`]).
pub(crate) fn instance_gains_block(card: &Card) -> bool {
    with_model!(card.kind, m => m.instance_gains_block(card))
}

/// `GetResultLocationForCardPlay` of `kind` ([`CardModel::result_location_for_card_play`]).
pub(crate) fn result_location_for_card_play(
    kind: CardKind,
    base: ResultLocation,
) -> ResultLocation {
    with_model!(kind, m => m.result_location_for_card_play(base))
}

/// `CardModel.IsPlayable`.
pub(crate) fn is_playable(s: &CombatState, card: CardId) -> bool {
    with_model!(s.card(card).kind, m => m.is_playable(s, card))
}

/// `CardModel.ShouldGlowGoldInternal` ([`CardModel::glows_gold`]).
pub(crate) fn glows_gold(s: &CombatState, card: CardId) -> bool {
    with_model!(s.card(card).kind, m => m.glows_gold(s, card))
}

/// `CardModel.OnPlay`.
pub(crate) fn on_play(eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
    let kind = eng.s.card(play.card).kind;
    kind.model().on_play(eng, play)
}

/// `CardModel.OnTurnEndInHandWrapper`.
pub(crate) fn on_turn_end_in_hand(eng: &mut Engine<'_>, card: CardId) -> Flow {
    let kind = eng.s.card(card).kind;
    kind.model().on_turn_end_in_hand(eng, card)
}

/// `OnUpgrade`, the instance-state part ([`CardModel::on_upgrade`]).
pub(crate) fn on_upgrade(card: &mut Card) {
    card.kind.model().on_upgrade(card);
}

/// `CardModel.AfterCreated`.
pub(crate) fn after_created(s: &mut CombatState, card: CardId) {
    let kind = s.card(card).kind;
    kind.model().after_created(s, card);
}

/// `AfterDowngraded` of `card`.
pub(crate) fn after_downgraded(s: &mut CombatState, card: CardId) {
    let kind = s.card(card).kind;
    kind.model().after_downgraded(s, card);
}

#[cfg(test)]
mod sort_key_tests {
    use super::*;
    use crate::combat::CardId;

    #[test]
    fn sort_key_orders_as_compare() {
        let cards: Vec<Card> = CardKind::ALL
            .into_iter()
            .flat_map(|kind| (0..3).map(move |level| Card::new(CardId(0), kind, level)))
            .collect();
        for a in &cards {
            for b in &cards {
                assert_eq!(sort_key(a).cmp(&sort_key(b)), compare(a, b), "{a:?} {b:?}");
            }
        }
    }
}
