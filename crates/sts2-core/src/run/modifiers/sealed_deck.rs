//! The `SealedDeck` modifier.
//!
//! `SealedDeck` (`Models.Modifiers/SealedDeck.cs`): the deck starts empty; Neow's option creates
//! 30 distinct cards (`CreateForReward` of 30: character pool, `RegularEncounter` odds rolled
//! with the pity change forced, `IsCardReward`, no upgrade roll), shown sorted by rarity then
//! title, and the player keeps 10; then `PandorasBox` leaves the relic bags.

use crate::run::odds::{CardRarity, CardRarityOddsType};
use crate::run::rewards::{
    CardCreation, CardFilter, CardOdds, CardPool, CreationFlags, create_card_reward,
};
use crate::run::{RunCard, RunState};

/// The cards created.
pub const OFFERED: usize = 30;

/// The cards kept.
pub const KEPT: usize = 10;

/// `CreateForReward(player, 30, options)`, sorted by rarity (the enum's order), then id: the selection's own order, which the game
/// shows sorted by title ([`display_order`]).
pub(super) fn offer(s: &mut RunState) -> Vec<RunCard> {
    let options = CardCreation {
        pool: CardPool::Character,
        odds: CardOdds::Encounter(CardRarityOddsType::RegularEncounter),
        filter: CardFilter::Any,
        upgrade_roll: false,
        is_card_reward: true,
        flags: CreationFlags::NONE,
    };
    let mut cards = create_card_reward(s, OFFERED, options);
    cards.sort_by_key(|c| (rarity_order(c), c.id));
    cards
}

fn rarity_order(card: &RunCard) -> u8 {
    match card.info().map(|i| i.rarity) {
        Some(CardRarity::Basic) => 1,
        Some(CardRarity::Common) => 2,
        Some(CardRarity::Uncommon) => 3,
        Some(CardRarity::Rare) => 4,
        _ => 5,
    }
}

/// The order the game's grid shows the 30 cards in (`SealedDeck.CompareCards`).
///
/// By rarity, then by `title` compared as the culture does (case apart, a lowercase letter before its capital).
/// Returns indices into `cards`.
pub fn display_order(cards: &[RunCard], title: impl Fn(RunCard) -> String) -> Vec<usize> {
    let keyed: Vec<(u8, String)> = cards
        .iter()
        .map(|&c| (rarity_order(&c), title(c)))
        .collect();
    let culture = |t: &str| -> (Vec<char>, Vec<bool>) {
        (
            t.chars().flat_map(char::to_lowercase).collect(),
            t.chars().map(char::is_uppercase).collect(),
        )
    };
    let mut order: Vec<usize> = (0..cards.len()).collect();
    order.sort_by_key(|&i| (keyed[i].0, culture(&keyed[i].1)));
    order
}
