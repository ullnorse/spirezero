//! `Nimble` (`Models.Enchantments/Nimble.cs`): `+Amount` to the block the card gains, before
//! every other block modifier. Only for cards that gain block (`CardModel.GainsBlock`).

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::content::enchantments::{Enchantment, EnchantmentModel};
use crate::decimal::Decimal;

/// `+Amount` block on a card that gains block.
pub struct Nimble;

impl EnchantmentModel for Nimble {
    fn name(&self) -> &'static str {
        "Nimble"
    }

    fn can_enchant_extra(&self, card: &Card) -> bool {
        card.gains_block()
    }

    fn enchant_block_additive(&self, enchantment: Enchantment, _block: Decimal) -> Decimal {
        Decimal::from(enchantment.amount)
    }
}

#[sts2_macros::hooks]
impl Hooks for Nimble {
    type Me = CardId;
}
