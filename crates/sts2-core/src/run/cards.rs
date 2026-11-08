//! Run-level cards: the deck's card ids, the colorless pool in the game's order (the character
//! pools are in [`crate::content::characters`]), and the bridge to the combat engine's ported cards.

use super::RunError;
use super::odds::CardRarity::{self, Ancient, Basic, Common, Rare, Uncommon};
use crate::combat::state::Card;
use crate::combat::{CardId, DeckCard};
use crate::content::cards::CardType::{self, Attack, Curse, Power, Quest, Skill};
use crate::content::cards::{CardKind, Var};
use crate::content::characters::Character;
use crate::content::enchantments::{Enchantment, EnchantmentKind};
use crate::decimal::Decimal;

/// A card in the run deck: its id (`ModelId.Entry`), upgrade level and enchantment. Any card can
/// be in the deck; only cards the combat engine ports can be fought with (see
/// [`RunCard::to_combat`]).
///
/// Build one with [`RunCard::new`] (then set the other fields), so that its [`RunCard::kind`]
/// matches the id; don't change the id of a card.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunCard {
    /// `ModelId.Entry`.
    pub id: &'static str,
    /// The ported kind of `id`, looked up once ([`RunCard::kind`]).
    kind: Option<CardKind>,
    /// Times upgraded.
    pub upgrade_level: u8,
    /// `Enchantment`, carried into every combat.
    pub enchantment: Option<Enchantment>,
    /// The card's saved counter: `Guilty.CombatsSeen`, `Dowsing.RoomsEntered`,
    /// `TheScythe.IncreasedDamage` (its `CurrentDamage` is `13 + IncreasedDamage`), 1 for a
    /// `SpoilsMap` whose `SpoilsCoord` is set, `MadScience`'s type and rider
    /// (`mad_science::counter`); 0 otherwise.
    pub counter: u32,
    /// The upgrade level a downgraded deck card's calculated vars still show the
    /// `CalculationBase` of ([`Card::stale_calculated`]), until an upgrade or enchantment; its
    /// combat clone is fresh. `None` when they show the current one.
    pub calculated_level: Option<u8>,
}

/// As the derive would print it without `kind` (a function of `id`), so that the run's text is
/// the same as before the field.
impl std::fmt::Debug for RunCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            id,
            kind: _,
            upgrade_level,
            enchantment,
            counter,
            calculated_level,
        } = self;
        f.debug_struct("RunCard")
            .field("id", id)
            .field("upgrade_level", upgrade_level)
            .field("enchantment", enchantment)
            .field("counter", counter)
            .field("calculated_level", calculated_level)
            .finish()
    }
}

impl RunCard {
    /// An unupgraded card.
    #[must_use]
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            kind: CardKind::from_id(id),
            upgrade_level: 0,
            enchantment: None,
            counter: 0,
            calculated_level: None,
        }
    }

    /// `CardModel.CompareTo`: model id (ordinal), then upgrade level.
    #[must_use]
    pub fn compare_to(&self, other: &Self) -> std::cmp::Ordering {
        self.id
            .cmp(other.id)
            .then(self.upgrade_level.cmp(&other.upgrade_level))
    }

    /// `CardCmd.Upgrade`: one level up, if upgradable.
    pub fn upgrade(&mut self) {
        if self.is_upgradable() {
            self.upgrade_level += 1;
            self.calculated_level = None;
        }
    }

    /// The pool entry of this card, if it is a character pool card (of any character).
    #[must_use]
    pub fn info(self) -> Option<&'static PoolCard> {
        Character::pool_card(self.id).map(|(_, card)| card)
    }

    /// `Rarity == Basic` (only character pool cards are Basic).
    #[must_use]
    pub fn is_basic(self) -> bool {
        self.info().is_some_and(|i| i.rarity == Basic)
    }

    /// `IsRemovable`: not `Eternal` (printed, or added by `TezcatarasEmber`, the only
    /// enchantment that adds it). For a deck card this is also `IsTransformable`.
    #[must_use]
    pub fn is_removable(self) -> bool {
        !ETERNAL_CARDS.contains(&self.id)
            && !self
                .enchantment
                .is_some_and(|e| e.kind == EnchantmentKind::TezcatarasEmber)
    }

    /// The `Type` of a card the run layer knows (character or colorless pool, an event card, or
    /// any card the combat engine ports); `None` for any other card.
    #[must_use]
    pub fn card_type(self) -> Option<CardType> {
        self.info()
            .or_else(|| COLORLESS_CARD_POOL.iter().find(|c| c.id == self.id))
            .map(|c| c.card_type)
            .or_else(|| {
                EVENT_DECK_CARDS
                    .iter()
                    .find(|(id, _)| *id == self.id)
                    .map(|&(_, t)| t)
            })
            // Any other card the combat engine ports (`Peck`, `ToricToughness`, curses).
            .or_else(|| self.card().map(|c| c.card_type()))
    }

    /// `IsUpgradable`: below `MaxUpgradeLevel`, which is 1 for every character pool card (none
    /// overrides it); a card the combat engine ports says its own (`NeowsFury` 1, curses 0).
    #[must_use]
    pub fn is_upgradable(self) -> bool {
        let max = self.to_combat().map_or_else(
            || u8::from(self.info().is_some()),
            |card| card.kind.model().max_upgrade_level(),
        );
        self.upgrade_level < max
    }

    /// Whether this is a character's Basic Strike (`Rarity` Basic and `CardTag.Strike`): every
    /// character's is its `Strike<Character>`, the only Basic card with the tag.
    #[must_use]
    pub fn is_strike(self) -> bool {
        self.is_basic() && self.id.starts_with("STRIKE_")
    }

    /// Whether this is a character's Basic Defend (`Rarity` Basic and `CardTag.Defend`); see
    /// [`RunCard::is_strike`].
    #[must_use]
    pub fn is_defend(self) -> bool {
        self.is_basic() && self.id.starts_with("DEFEND_")
    }

    /// The card's kind, or `None` if it is not ported: `CardKind::from_id(id)`, without a
    /// lookup.
    #[must_use]
    pub fn kind(self) -> Option<CardKind> {
        debug_assert_eq!(
            self.kind,
            CardKind::from_id(self.id),
            "{} changed its id",
            self.id
        );
        self.kind
    }

    /// The combat engine's card, or `None` if it is not ported.
    #[must_use]
    pub fn to_combat(self) -> Option<DeckCard> {
        let kind = self.kind()?;
        Some(DeckCard {
            kind,
            upgrade_level: self.upgrade_level,
            enchantment: self.enchantment,
            counter: self.counter,
        })
    }

    /// The deck card itself as a card instance (unlike its combat clone, with
    /// [`RunCard::calculated_level`]), or `None` if it is not ported.
    #[must_use]
    pub fn card(self) -> Option<Card> {
        let mut card = Card::from_deck(CardId(0), self.to_combat()?);
        if let Some(level) = self.calculated_level {
            let old = Self {
                upgrade_level: level,
                calculated_level: None,
                ..self
            };
            card.keep_stale_calculated(old.card()?.calculation_base());
        }
        Some(card)
    }

    /// `CardCmd.Downgrade` out of combat (`DowngradeInternal`, `CardModel.cs:2149-2162`): back to
    /// level 0, keeping the enchantment; a calculated var keeps the upgraded base unless the
    /// enchantment's `ModifyCard` recalculates it.
    pub fn downgrade(&mut self) {
        let level = self.upgrade_level;
        self.upgrade_level = 0;
        self.calculated_level = None;
        if self.enchantment.is_none() {
            self.calculated_level = Some(level);
            if self.card().is_none_or(|c| c.stale_calculated.is_none()) {
                self.calculated_level = None;
            }
        }
    }

    /// The deck card's `DynamicVars` in declaration order, or `None` if it is not ported. Out of
    /// combat a calculated var's `Calculate(null)` is its `CalculationBase` (the multiplier is 0
    /// for a card without a `CombatState`, `CalculatedVar.cs:62-70`), even while a fight runs.
    #[must_use]
    pub fn vars(self) -> Option<Vec<DeckVar>> {
        let card = self.card()?;
        Some(
            card.kind
                .vars()
                .iter()
                .map(|v| DeckVar {
                    name: v.name,
                    value: card.var(v.name),
                    calculated: v.multiplier.map(|_| card.var(Var::CalculationBase)),
                })
                .collect(),
        )
    }

    /// The deck card's vars as its text shows them (`UpdateDynamicVarPreview` with a run and no
    /// combat: the enchantment's damage and block changes, no global hooks), in declaration
    /// order, or `None` if it is not ported. See [`Card::preview_out_of_combat`].
    #[must_use]
    pub fn preview(self) -> Option<Vec<(Var, Decimal)>> {
        Some(self.card()?.preview_out_of_combat().collect())
    }

    /// Its enchantment's vars as the card previews them outside a fight
    /// ([`Card::enchantment_preview_out_of_combat`]); `None` if the card is not ported.
    #[must_use]
    pub fn enchantment_preview(self) -> Option<Vec<(&'static str, Decimal)>> {
        Some(self.card()?.enchantment_preview_out_of_combat())
    }

    /// `EnchantmentModel.CanEnchant(card)` for this deck card.
    ///
    /// # Errors
    /// [`RunError::UnsupportedCard`] if the card is not ported: its keywords and cost decide.
    pub fn can_enchant(self, kind: EnchantmentKind) -> Result<bool, RunError> {
        let deck = self.to_combat().ok_or(RunError::UnsupportedCard(self.id))?;
        Ok(kind.can_enchant(&Card::from_deck(CardId(0), deck)))
    }
}

/// One of a deck card's vars ([`RunCard::vars`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeckVar {
    /// The var's name.
    pub name: Var,
    /// `BaseValue`.
    pub value: Decimal,
    /// `Calculate(null)`, for a calculated var.
    pub calculated: Option<Decimal>,
}

/// One card of a pool: id, `Rarity`, `Type` and whether it is `MultiplayerOnly`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PoolCard {
    /// `ModelId.Entry`.
    pub id: &'static str,
    /// `Rarity`.
    pub rarity: CardRarity,
    /// `Type`.
    pub card_type: CardType,
    /// `MultiplayerConstraint` is `MultiplayerOnly`.
    pub multiplayer_only: bool,
}

pub(crate) const fn pool_card(
    id: &'static str,
    rarity: CardRarity,
    card_type: CardType,
    multiplayer_only: bool,
) -> PoolCard {
    PoolCard {
        id,
        rarity,
        card_type,
        multiplayer_only,
    }
}

/// `ColorlessCardPool.GenerateAllCards()` in declaration order (every colorless epoch revealed).
pub const COLORLESS_CARD_POOL: [PoolCard; 65] = [
    pool_card("ALCHEMIZE", Rare, Skill, false),
    pool_card("ANOINTED", Rare, Skill, false),
    pool_card("AUTOMATION", Uncommon, Power, false),
    pool_card("BEACON_OF_HOPE", Rare, Power, true),
    pool_card("BEAT_DOWN", Rare, Skill, false),
    pool_card("BELIEVE_IN_YOU", Uncommon, Skill, true),
    pool_card("BOLAS", Rare, Attack, false),
    pool_card("CALAMITY", Rare, Power, false),
    pool_card("CATASTROPHE", Uncommon, Skill, false),
    pool_card("COORDINATE", Uncommon, Skill, true),
    pool_card("DARK_SHACKLES", Uncommon, Skill, false),
    pool_card("DISCOVERY", Uncommon, Skill, false),
    pool_card("DRAMATIC_ENTRANCE", Uncommon, Attack, false),
    pool_card("ENTROPY", Rare, Power, false),
    pool_card("EQUILIBRIUM", Uncommon, Skill, false),
    pool_card("ETERNAL_ARMOR", Rare, Power, false),
    pool_card("FASTEN", Uncommon, Power, false),
    pool_card("FINESSE", Uncommon, Skill, false),
    pool_card("FISTICUFFS", Uncommon, Attack, false),
    pool_card("FLASH_OF_STEEL", Uncommon, Attack, false),
    pool_card("GANG_UP", Uncommon, Attack, true),
    pool_card("GOLD_AXE", Rare, Attack, false),
    pool_card("HAND_OF_GREED", Rare, Attack, false),
    pool_card("HIDDEN_GEM", Rare, Skill, false),
    pool_card("HUDDLE_UP", Uncommon, Skill, true),
    pool_card("IMPATIENCE", Uncommon, Skill, false),
    pool_card("INTERCEPT", Uncommon, Skill, true),
    pool_card("JACK_OF_ALL_TRADES", Uncommon, Skill, false),
    pool_card("JACKPOT", Rare, Attack, false),
    pool_card("KNOCKDOWN", Rare, Attack, true),
    pool_card("LIFT", Uncommon, Skill, true),
    pool_card("MASTER_OF_STRATEGY", Rare, Skill, false),
    pool_card("MAYHEM", Rare, Power, false),
    pool_card("MIMIC", Rare, Skill, true),
    pool_card("MIND_BLAST", Uncommon, Attack, false),
    pool_card("NOSTALGIA", Rare, Power, false),
    pool_card("OMNISLICE", Uncommon, Attack, false),
    pool_card("PANACHE", Uncommon, Power, false),
    pool_card("PANIC_BUTTON", Uncommon, Skill, false),
    pool_card("PREP_TIME", Uncommon, Power, false),
    pool_card("PRODUCTION", Uncommon, Skill, false),
    pool_card("PROLONG", Uncommon, Skill, false),
    pool_card("PROWESS", Uncommon, Power, false),
    pool_card("PURITY", Uncommon, Skill, false),
    pool_card("RALLY", Rare, Skill, true),
    pool_card("REND", Rare, Attack, false),
    pool_card("RESTLESSNESS", Uncommon, Skill, false),
    pool_card("ROLLING_BOULDER", Rare, Power, false),
    pool_card("SALVO", Uncommon, Attack, false),
    pool_card("SCRAWL", Rare, Skill, false),
    pool_card("SECRET_TECHNIQUE", Rare, Skill, false),
    pool_card("SECRET_WEAPON", Rare, Skill, false),
    pool_card("SEEKER_STRIKE", Uncommon, Attack, false),
    pool_card("SHOCKWAVE", Uncommon, Skill, false),
    pool_card("SPLASH", Rare, Skill, false),
    pool_card("STRATAGEM", Uncommon, Power, false),
    pool_card("TAG_TEAM", Uncommon, Attack, true),
    pool_card("THE_BALL", Uncommon, Attack, true),
    pool_card("THE_BOMB", Uncommon, Skill, false),
    pool_card("THE_GAMBIT", Rare, Skill, false),
    pool_card("THINKING_AHEAD", Uncommon, Skill, false),
    pool_card("THRUMMING_HATCHET", Uncommon, Attack, false),
    pool_card("ULTIMATE_DEFEND", Uncommon, Skill, false),
    pool_card("ULTIMATE_STRIKE", Uncommon, Attack, false),
    pool_card("VOLLEY", Uncommon, Attack, false),
];

/// Deck cards from outside the reward pools, with their `Type`.
///
/// Events, Neow and the ascension add them: curses (`Rarity` Curse) and quest cards (`Rarity`
/// Quest). All have `MaxUpgradeLevel` 0, so none is upgradable.
pub const EVENT_DECK_CARDS: [(&str, CardType); 10] = [
    ("ASCENDERS_BANE", Curse),
    ("CLUMSY", Curse),
    ("GREED", Curse),
    ("GUILTY", Curse),
    ("INJURY", Curse),
    ("POOR_SLEEP", Curse),
    ("SPORE_MIND", Curse),
    ("BYRDONIS_EGG", Quest),
    ("DOWSING", Quest),
    ("SPOILS_MAP", Quest),
];

/// Cards whose `CanonicalKeywords` hold `Eternal` (`IsRemovable` is false).
pub(crate) const ETERNAL_CARDS: [&str; 7] = [
    "ASCENDERS_BANE",
    "BAD_LUCK",
    "CURSE_OF_THE_BELL",
    "ENTHRALLED",
    "FOLLY",
    "FORBIDDEN_GRIMOIRE",
    "GREED",
];

/// `CardFactory.GetDefaultTransformationOptions(original, false)` as ids in pool order.
///
/// `Factories/CardFactory.cs:170-208`: the original's pool (its character's, whoever owns it),
/// or the colorless pool for a Quest card or an Event, Ancient or Token one (`ByrdSwoop`,
/// `NeowsFury`, `ToricToughness`), Common, Uncommon and Rare cards only (the original is not a
/// Status or Curse here), minus the original. `None` for a card whose pool is not modeled
/// (curses and status cards).
///
/// # Panics
/// Never: a card with a pool entry has a character.
#[must_use]
pub fn transformation_options(original: RunCard) -> Option<Vec<&'static str>> {
    let colorless = || unlocked_colorless_cards().collect::<Vec<_>>();
    let pool: Vec<&PoolCard> = match original.info() {
        Some(info) if info.rarity != Ancient => {
            let character = Character::of_card(original.id).expect("a pool card has a character");
            unlocked_cards(character).collect()
        }
        Some(_) => colorless(),
        None if COLORLESS_CARD_POOL.iter().any(|c| c.id == original.id)
            || original.card_type() == Some(Quest)
            || original.to_combat().is_some_and(|c| {
                use crate::content::cards::CardRarity as R;
                matches!(c.kind.rarity(), R::Event | R::Ancient | R::Token)
            }) =>
        {
            colorless()
        }
        None => return None,
    };
    Some(
        pool.into_iter()
            .filter(|c| matches!(c.rarity, Common | Uncommon | Rare) && c.id != original.id)
            .map(|c| c.id)
            .collect(),
    )
}

/// `ColorlessCardPool.GetUnlockedCards` in single player: the pool minus `MultiplayerOnly`.
pub fn unlocked_colorless_cards() -> impl Iterator<Item = &'static PoolCard> {
    COLORLESS_CARD_POOL.iter().filter(|c| !c.multiplayer_only)
}

/// `CardPoolModel.GetUnlockedCards` of `character`'s pool for a single-player run with every
/// epoch revealed: the pool minus `MultiplayerOnly` cards, in pool order.
pub fn unlocked_cards(character: Character) -> impl Iterator<Item = &'static PoolCard> {
    character.card_pool().iter().filter(|c| !c.multiplayer_only)
}

/// `character.StartingDeck`, in order.
#[must_use]
pub fn starter_deck(character: Character) -> Vec<RunCard> {
    character
        .starting_deck()
        .iter()
        .map(|&id| RunCard::new(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::cards::mad_science;

    fn calculated_doom(card: RunCard) -> (Decimal, Option<Decimal>) {
        let vars = card.vars().expect("ported");
        let doom = vars
            .iter()
            .find(|v| v.name == Var::Named("CalculatedDoom"))
            .expect("NoEscape has CalculatedDoom");
        (doom.value, doom.calculated)
    }

    #[test]
    fn a_downgraded_calculated_var_keeps_its_upgraded_base_value() {
        // `DowngradeInternal` clones the new vars against the old ones (`CardModel.cs:2154`):
        // NoEscape+1's `CalculatedDoom` shows `[15, 10]` until an upgrade recalculates it.
        let mut card = RunCard::new("NO_ESCAPE");
        card.upgrade();
        card.downgrade();
        assert_eq!(card.upgrade_level, 0);
        assert_eq!(
            calculated_doom(card),
            (Decimal::from(15), Some(Decimal::from(10)))
        );
        card.upgrade();
        assert_eq!(card.calculated_level, None);
        assert_eq!(
            calculated_doom(card),
            (Decimal::from(15), Some(Decimal::from(15)))
        );
        // Downgrading an unupgraded card changes nothing.
        let mut fresh = RunCard::new("NO_ESCAPE");
        fresh.downgrade();
        assert_eq!(fresh, RunCard::new("NO_ESCAPE"));
        // An enchantment's `ModifyCard` recalculates (`EnchantmentModel.cs:363`).
        let mut enchanted = RunCard::new("NO_ESCAPE");
        enchanted.upgrade();
        enchanted.enchantment = Some(Enchantment::new(EnchantmentKind::Swift, 1));
        enchanted.downgrade();
        assert_eq!(enchanted.calculated_level, None);
        assert_eq!(calculated_doom(enchanted).0, Decimal::from(10));
    }

    #[test]
    fn a_skill_mad_science_is_not_corrupted_enchantable() {
        // `MadScience.Type` is its saved `TinkerTimeType`, which `CanEnchant` reads.
        let mad_science = |ty| {
            let mut card = RunCard::new("MAD_SCIENCE");
            card.counter = mad_science::counter(mad_science::Data {
                tinker_time_type: Some(ty),
                tinker_time_rider: mad_science::Rider::None,
            });
            card
        };
        let corrupted = EnchantmentKind::Corrupted;
        assert_eq!(mad_science(Skill).can_enchant(corrupted), Ok(false));
        assert_eq!(mad_science(Attack).can_enchant(corrupted), Ok(true));
    }

    #[test]
    fn ported_pool_cards_map_to_combat() {
        // A pool card maps to the combat card with its entry (status and token cards such as
        // Slimed or Dazed are ported for combat but are not in the pool).
        let in_pool = CardKind::ALL
            .into_iter()
            .filter(|k| RunCard::new(k.entry()).info().is_some());
        for kind in in_pool {
            assert_eq!(
                RunCard::new(kind.entry()).to_combat().map(|c| c.kind),
                Some(kind)
            );
        }
        assert_eq!(unlocked_cards(Character::Necrobinder).count(), 91 - 5);
        assert!(
            starter_deck(Character::Necrobinder)
                .iter()
                .all(|c| c.to_combat().is_some())
        );
    }
}
