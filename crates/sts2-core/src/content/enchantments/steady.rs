//! `Steady` (`Models.Enchantments/Steady.cs`): `OnEnchant` adds the Retain keyword.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::content::cards::CardKeyword;
use crate::content::enchantments::EnchantmentModel;

/// Adds Retain.
pub struct Steady;

impl EnchantmentModel for Steady {
    fn name(&self) -> &'static str {
        "Steady"
    }

    fn on_enchant(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Retain);
    }
}

#[sts2_macros::hooks]
impl Hooks for Steady {
    type Me = CardId;
}
