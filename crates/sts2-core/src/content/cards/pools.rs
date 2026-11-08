//! Card pools as combat card generation sees them.
//!
//! `CardPoolModel.GetUnlockedCards` for a single-player run with every epoch revealed: the
//! canonical data `CardFactory` filters on, for every pool card, ported or not. The combat draws over the whole pool; only the cards it
//! creates must be ported [`CardKind`]s.

use crate::combat::{CardId, Engine, Flow, Unsupported};
use crate::content::cards::CardKind;
use crate::content::cards::CardRarity::{
    self, Ancient, Basic, Common, Event, Quest, Rare, Uncommon,
};
use crate::content::cards::CardType::{
    self, Attack, Curse as CurseType, Power, Skill, Status as StatusType,
};
use crate::content::characters::Character;
use crate::rng::RunRngType;
use crate::run::cards::unlocked_cards;
use crate::run::odds;

/// One card of a pool with the canonical data generation filters on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PoolCard {
    /// `ModelId.Entry`.
    pub id: &'static str,
    /// `Rarity`.
    pub rarity: CardRarity,
    /// `Type`.
    pub card_type: CardType,
    /// `CanonicalKeywords` contains `Ethereal`.
    pub ethereal: bool,
    /// `CanBeGeneratedInCombat`.
    pub can_be_generated_in_combat: bool,
}

/// The combat card of a picked pool card, or the [`Unsupported`] stop if it is not ported.
pub(crate) fn ported(card: PoolCard) -> Flow<CardKind> {
    card.kind()
        .map_or_else(|| Unsupported::card(card.id).stop(), Ok)
}

impl PoolCard {
    /// The combat card of this pool card, if it is ported.
    #[must_use]
    pub fn kind(self) -> Option<CardKind> {
        CardKind::from_id(self.id)
    }
}

/// `character.CardPool.GetUnlockedCards`, in pool order.
#[must_use]
pub fn character(character: Character) -> Vec<PoolCard> {
    unlocked_cards(character)
        .map(|c| PoolCard {
            id: c.id,
            rarity: match c.rarity {
                odds::CardRarity::Basic => Basic,
                odds::CardRarity::Common => Common,
                odds::CardRarity::Uncommon => Uncommon,
                odds::CardRarity::Rare => Rare,
                odds::CardRarity::Ancient => Ancient,
            },
            card_type: c.card_type,
            ethereal: character.ethereal_cards().contains(&c.id),
            can_be_generated_in_combat: !character.not_generated_cards().contains(&c.id),
        })
        .collect()
}

/// `ColorlessCardPool.GetUnlockedCards`, in pool order (`GenerateAllCards` without the
/// `MultiplayerOnly` cards). No Colorless card is canonically Ethereal.
#[must_use]
pub fn colorless() -> Vec<PoolCard> {
    COLORLESS_CARD_POOL
        .iter()
        .filter(|(_, multiplayer_only)| !multiplayer_only)
        .map(|&(card, _)| card)
        .collect()
}

/// `StatusCardPool.GenerateAllCards` in declaration order, with `CanBeGeneratedInCombat`
/// (false for `FranticEscape` and `Soot`).
const STATUS_CARD_POOL: [(&str, bool); 12] = [
    ("BECKON", true),
    ("BURN", true),
    ("DAZED", true),
    ("DEBRIS", true),
    ("FRANTIC_ESCAPE", false),
    ("INFECTION", true),
    ("WITHER", true),
    ("SLIMED", true),
    ("SOOT", false),
    ("TOXIC", true),
    ("VOID", true),
    ("WOUND", true),
];

/// `CurseCardPool.GenerateAllCards` in declaration order, with `CanBeGeneratedInCombat`
/// (false for `AscendersBane`).
const CURSE_CARD_POOL: [(&str, bool); 18] = [
    ("ASCENDERS_BANE", false),
    ("BAD_LUCK", true),
    ("CLUMSY", true),
    ("CURSE_OF_THE_BELL", true),
    ("DEBT", true),
    ("DECAY", true),
    ("DOUBT", true),
    ("ENTHRALLED", true),
    ("FOLLY", true),
    ("GREED", true),
    ("GUILTY", true),
    ("INJURY", true),
    ("NORMALITY", true),
    ("POOR_SLEEP", true),
    ("REGRET", true),
    ("SHAME", true),
    ("SPORE_MIND", true),
    ("WRITHE", true),
];

/// `StatusCardPool.GetUnlockedCards`, in pool order (no Status is `MultiplayerOnly`).
#[must_use]
pub fn status() -> Vec<PoolCard> {
    pool_of(&STATUS_CARD_POOL, CardRarity::Status, StatusType)
}

/// `CurseCardPool.GetUnlockedCards`, in pool order (no Curse is `MultiplayerOnly`).
#[must_use]
pub fn curse() -> Vec<PoolCard> {
    pool_of(&CURSE_CARD_POOL, CardRarity::Curse, CurseType)
}

fn pool_of(
    cards: &[(&'static str, bool)],
    rarity: CardRarity,
    card_type: CardType,
) -> Vec<PoolCard> {
    cards
        .iter()
        .map(|&(id, can_be_generated_in_combat)| PoolCard {
            id,
            rarity,
            card_type,
            ethereal: false,
            can_be_generated_in_combat,
        })
        .collect()
}

/// `Token` pool (`TokenCardPool.GenerateAllCards`), whose `IsColorless` is true.
const TOKEN_CARD_POOL: [&str; 14] = [
    "DISINTEGRATION",
    "FUEL",
    "GIANT_ROCK",
    "LUMINESCE",
    "MIND_ROT",
    "MINION_DIVE_BOMB",
    "MINION_SACRIFICE",
    "MINION_STRIKE",
    "SHIV",
    "SLOTH",
    "SOUL",
    "SOVEREIGN_BLADE",
    "SWEEPING_GAZE",
    "WASTE_AWAY",
];

/// `EventCardPool.GenerateAllCards` cards whose `VisualCardPool` is the pool itself
/// (`IsColorless` true); the ten that override it with a character pool are left out
/// (`Caltrops`, `Clash`, `Distraction`, `DualWield`, `Entrench`, `HelloWorld`, `Outmaneuver`,
/// `Rebound`, `RipAndTear`, `Stack`).
const EVENT_CARD_POOL: [&str; 18] = [
    "ABUNDANCE",
    "APOTHEOSIS",
    "APPARITION",
    "BRIGHTEST_FLAME",
    "BYRD_SWOOP",
    "ENLIGHTENMENT",
    "EXTERMINATE",
    "FEEDING_FRENZY",
    "MAD_SCIENCE",
    "MAUL",
    "METAMORPHOSIS",
    "NEOWS_FURY",
    "PECK",
    "RELAX",
    "SQUASH",
    "TORIC_TOUGHNESS",
    "WISH",
    "WHISTLE",
];

/// `CardModel.VisualCardPool.IsColorless` for the card with `ModelId.Entry` `entry`.
///
/// True when its visual pool is the Colorless, Token, Event or Deprecated pool
/// (`CardPoolModel.IsColorless`); Status, Curse, Quest and the character pools are not.
#[must_use]
pub fn is_visual_pool_colorless(entry: &str) -> bool {
    COLORLESS_CARD_POOL.iter().any(|(card, _)| card.id == entry)
        || TOKEN_CARD_POOL.contains(&entry)
        || EVENT_CARD_POOL.contains(&entry)
        || entry == "DEPRECATED_CARD"
}

/// `CardFactory.FilterForCombat`: generatable cards that are not Basic, Ancient or Event (the
/// pools have no duplicates, so `Distinct` changes nothing).
#[must_use]
pub fn filter_for_combat(cards: &[PoolCard]) -> Vec<PoolCard> {
    cards
        .iter()
        .copied()
        .filter(|c| c.can_be_generated_in_combat && !matches!(c.rarity, Basic | Ancient | Event))
        .collect()
}

/// `CardFactory.GetForCombat(player, cards, count, Rng.CombatCardGeneration)`
/// (`CardFactory.cs:140-152`): `FilterForCombat`, then `count` picks with `NextItem` (the same card
/// can come up twice), each created with `CombatState.CreateCard` (in no pile; add them with
/// [`Engine::add_generated_card_to_combat`]).
///
/// A picked card that is not ported stops the combat ([`Unsupported`]) after its draw.
///
/// # Panics
/// If no card passes the filter.
pub(crate) fn get_for_combat(
    eng: &mut Engine<'_>,
    cards: &[PoolCard],
    count: usize,
) -> Flow<Vec<CardId>> {
    let options = filter_for_combat(cards);
    (0..count)
        .map(|_| {
            let picked = *eng.s.rng[RunRngType::CombatCardGeneration]
                .next_item(&options)
                .expect("GetForCombat has cards to pick from");
            Ok(eng.create_card(ported(picked)?))
        })
        .collect()
}

/// `CardCmd.TransformToRandom(original, Rng.CombatCardSelection)` for a combat card
/// (`CardCmd.cs:325`, `CardFactory.CreateRandomCardForTransform`): a random card of the original's
/// pool replaces it in its pile. The pool is the Colorless one for a Quest, Event, Ancient or Token
/// original, else the original's own. Unless the original is a Status or Curse the options are
/// Common, Uncommon and Rare cards; in combat only generatable ones; never the original's own id.
///
/// A picked card that is not ported stops the combat ([`Unsupported`]) after its draw, and an
/// original whose pool is not modeled (not a character's, Colorless, Status or Curse) stops it
/// before any.
///
/// # Panics
/// If no option is left (`GetFilteredTransformationOptions` throws).
pub(crate) fn transform_to_random(eng: &mut Engine<'_>, original: CardId) -> Flow {
    // `CardCmd.Transform` returns before choosing the replacement once the combat is ending.
    if eng.hooked().is_ending() {
        return Ok(());
    }
    let kind = eng.s.card(original).kind;
    let rarity = kind.rarity();
    let in_pool = |pool: &[PoolCard]| pool.iter().any(|c| c.id == kind.entry());
    let pool =
        if matches!(rarity, Quest | Event | Ancient | CardRarity::Token) || in_pool(&colorless()) {
            colorless()
        } else if let Some(owner) = Character::of_card(kind.entry()) {
            character(owner)
        } else if rarity == CardRarity::Status {
            status()
        } else if rarity == CardRarity::Curse {
            curse()
        } else {
            // A card of no modeled pool stops the combat, naming the original.
            return Unsupported::card(kind.entry()).stop();
        };
    let options: Vec<PoolCard> = pool
        .into_iter()
        .filter(|c| {
            (matches!(rarity, CardRarity::Status | CardRarity::Curse)
                || matches!(c.rarity, Common | Uncommon | Rare))
                && c.can_be_generated_in_combat
                && c.id != kind.entry()
        })
        .collect();
    let picked = *eng.s.rng[RunRngType::CombatCardSelection]
        .next_item(&options)
        .expect("a card has transformation options");
    eng.transform_to(original, ported(picked)?)
}

/// `CardCmd.TransformTo<T>(original)` followed by `CardCmd.Upgrade(replacement)` when `upgraded`
/// (`Charge`; `Begone` and `Guards` upgrade their replacement before `CardCmd.Transform`, but
/// the engine creates it inside `transform_to`, so it is upgraded as soon as it takes the
/// original's place). Nothing is upgraded once the combat is ending, when `transform_to` does
/// nothing.
pub(crate) fn transform_to_upgraded(
    eng: &mut Engine<'_>,
    original: CardId,
    kind: CardKind,
    upgraded: bool,
) -> Flow {
    let pile = eng
        .s
        .pile_of(original)
        .expect("a transformed card is in a pile");
    let index = eng
        .s
        .player
        .pile(pile)
        .iter()
        .position(|&c| c == original)
        .expect("the card is in its pile");
    eng.transform_to(original, kind)?;
    if upgraded && eng.s.pile_of(original).is_none() {
        let replacement = eng.s.player.pile(pile)[index];
        eng.upgrade_card(replacement);
    }
    Ok(())
}

const fn colorless_card(
    id: &'static str,
    rarity: CardRarity,
    card_type: CardType,
    multiplayer_only: bool,
    can_be_generated_in_combat: bool,
) -> (PoolCard, bool) {
    (
        PoolCard {
            id,
            rarity,
            card_type,
            ethereal: false,
            can_be_generated_in_combat,
        },
        multiplayer_only,
    )
}

/// `ColorlessCardPool.GenerateAllCards()` in declaration order, with whether each card is
/// `MultiplayerOnly`.
const COLORLESS_CARD_POOL: [(PoolCard, bool); 65] = [
    colorless_card("ALCHEMIZE", Rare, Skill, false, false),
    colorless_card("ANOINTED", Rare, Skill, false, true),
    colorless_card("AUTOMATION", Uncommon, Power, false, true),
    colorless_card("BEACON_OF_HOPE", Rare, Power, true, true),
    colorless_card("BEAT_DOWN", Rare, Skill, false, true),
    colorless_card("BELIEVE_IN_YOU", Uncommon, Skill, true, true),
    colorless_card("BOLAS", Rare, Attack, false, true),
    colorless_card("CALAMITY", Rare, Power, false, true),
    colorless_card("CATASTROPHE", Uncommon, Skill, false, true),
    colorless_card("COORDINATE", Uncommon, Skill, true, true),
    colorless_card("DARK_SHACKLES", Uncommon, Skill, false, true),
    colorless_card("DISCOVERY", Uncommon, Skill, false, true),
    colorless_card("DRAMATIC_ENTRANCE", Uncommon, Attack, false, true),
    colorless_card("ENTROPY", Rare, Power, false, true),
    colorless_card("EQUILIBRIUM", Uncommon, Skill, false, true),
    colorless_card("ETERNAL_ARMOR", Rare, Power, false, true),
    colorless_card("FASTEN", Uncommon, Power, false, true),
    colorless_card("FINESSE", Uncommon, Skill, false, true),
    colorless_card("FISTICUFFS", Uncommon, Attack, false, true),
    colorless_card("FLASH_OF_STEEL", Uncommon, Attack, false, true),
    colorless_card("GANG_UP", Uncommon, Attack, true, true),
    colorless_card("GOLD_AXE", Rare, Attack, false, true),
    colorless_card("HAND_OF_GREED", Rare, Attack, false, false),
    colorless_card("HIDDEN_GEM", Rare, Skill, false, false),
    colorless_card("HUDDLE_UP", Uncommon, Skill, true, true),
    colorless_card("IMPATIENCE", Uncommon, Skill, false, true),
    colorless_card("INTERCEPT", Uncommon, Skill, true, true),
    colorless_card("JACK_OF_ALL_TRADES", Uncommon, Skill, false, true),
    colorless_card("JACKPOT", Rare, Attack, false, true),
    colorless_card("KNOCKDOWN", Rare, Attack, true, true),
    colorless_card("LIFT", Uncommon, Skill, true, true),
    colorless_card("MASTER_OF_STRATEGY", Rare, Skill, false, true),
    colorless_card("MAYHEM", Rare, Power, false, true),
    colorless_card("MIMIC", Rare, Skill, true, true),
    colorless_card("MIND_BLAST", Uncommon, Attack, false, true),
    colorless_card("NOSTALGIA", Rare, Power, false, true),
    colorless_card("OMNISLICE", Uncommon, Attack, false, true),
    colorless_card("PANACHE", Uncommon, Power, false, true),
    colorless_card("PANIC_BUTTON", Uncommon, Skill, false, true),
    colorless_card("PREP_TIME", Uncommon, Power, false, true),
    colorless_card("PRODUCTION", Uncommon, Skill, false, true),
    colorless_card("PROLONG", Uncommon, Skill, false, true),
    colorless_card("PROWESS", Uncommon, Power, false, true),
    colorless_card("PURITY", Uncommon, Skill, false, true),
    colorless_card("RALLY", Rare, Skill, true, true),
    colorless_card("REND", Rare, Attack, false, true),
    colorless_card("RESTLESSNESS", Uncommon, Skill, false, true),
    colorless_card("ROLLING_BOULDER", Rare, Power, false, true),
    colorless_card("SALVO", Uncommon, Attack, false, true),
    colorless_card("SCRAWL", Rare, Skill, false, true),
    colorless_card("SECRET_TECHNIQUE", Rare, Skill, false, true),
    colorless_card("SECRET_WEAPON", Rare, Skill, false, true),
    colorless_card("SEEKER_STRIKE", Uncommon, Attack, false, true),
    colorless_card("SHOCKWAVE", Uncommon, Skill, false, true),
    colorless_card("SPLASH", Rare, Skill, false, true),
    colorless_card("STRATAGEM", Uncommon, Power, false, true),
    colorless_card("TAG_TEAM", Uncommon, Attack, true, true),
    colorless_card("THE_BALL", Uncommon, Attack, true, true),
    colorless_card("THE_BOMB", Uncommon, Skill, false, true),
    colorless_card("THE_GAMBIT", Rare, Skill, false, true),
    colorless_card("THINKING_AHEAD", Uncommon, Skill, false, true),
    colorless_card("THRUMMING_HATCHET", Uncommon, Attack, false, true),
    colorless_card("ULTIMATE_DEFEND", Uncommon, Skill, false, true),
    colorless_card("ULTIMATE_STRIKE", Uncommon, Attack, false, true),
    colorless_card("VOLLEY", Uncommon, Attack, false, true),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pools_have_their_single_player_cards() {
        assert_eq!(character(Character::Necrobinder).len(), 86);
        assert_eq!(colorless().len(), 65 - 12);
        assert_eq!(status().len(), 12);
        assert_eq!(curse().len(), 18);
        let ethereal: Vec<&str> = character(Character::Necrobinder)
            .iter()
            .filter(|c| c.ethereal)
            .map(|c| c.id)
            .collect();
        assert_eq!(ethereal, Character::Necrobinder.ethereal_cards());
        // Ported cards agree with the table.
        for card in Character::ALL.into_iter().flat_map(character) {
            if let Some(kind) = card.kind() {
                assert_eq!(kind.is_ethereal(), card.ethereal, "{}", card.id);
                assert_eq!(kind.card_type(), card.card_type, "{}", card.id);
                assert_eq!(kind.rarity(), card.rarity, "{}", card.id);
            }
        }
        for card in colorless().into_iter().chain(status()).chain(curse()) {
            if let Some(kind) = card.kind() {
                assert_eq!(kind.card_type(), card.card_type, "{}", card.id);
                assert_eq!(kind.rarity(), card.rarity, "{}", card.id);
            }
        }
    }
}
