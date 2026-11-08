//! `EggRelicHelper` (`Helpers.Models/EggRelicHelper.cs`): what `FrozenEgg`, `MoltenEgg` and
//! `ToxicEgg` share.

use crate::content::cards::CardType;
use crate::run::RunCard;

/// `UpgradeValidCards`: each upgradable card of `card_type` is replaced by an upgraded clone.
pub fn upgrade_valid_cards(cards: &mut [RunCard], card_type: CardType) {
    for card in cards {
        if card.card_type() == Some(card_type) && card.is_upgradable() {
            card.upgrade();
        }
    }
}

/// `TryModifyCardBeingAddedToDeck` of an egg: the upgraded clone of an upgradable card of
/// `card_type`.
#[must_use]
pub fn upgraded_clone(card: RunCard, card_type: CardType) -> Option<RunCard> {
    if card.card_type() != Some(card_type) || !card.is_upgradable() {
        return None;
    }
    let mut upgraded = card;
    upgraded.upgrade();
    Some(upgraded)
}
