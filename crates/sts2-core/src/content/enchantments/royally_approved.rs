//! `RoyallyApproved` (`Models.Enchantments/RoyallyApproved.cs`): attacks and skills only;
//! `OnEnchant` adds the Innate and Retain keywords.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::content::cards::{CardKeyword, CardType};
use crate::content::enchantments::EnchantmentModel;

/// Adds Innate and Retain.
pub struct RoyallyApproved;

impl EnchantmentModel for RoyallyApproved {
    fn name(&self) -> &'static str {
        "Royally Approved"
    }

    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        matches!(card_type, CardType::Attack | CardType::Skill)
    }

    fn on_enchant(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Innate);
        card.keywords.insert(CardKeyword::Retain);
    }
}

#[sts2_macros::hooks]
impl Hooks for RoyallyApproved {
    type Me = CardId;
}
