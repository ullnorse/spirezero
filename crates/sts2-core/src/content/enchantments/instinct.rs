//! `Instinct` (`Models.Enchantments/Instinct.cs`): attacks only; doubles the card's powered
//! attack damage.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, ValueProps};
use crate::content::cards::CardType;
use crate::content::enchantments::{Enchantment, EnchantmentModel};
use crate::decimal::Decimal;

/// `x2` damage on an attack.
pub struct Instinct;

impl EnchantmentModel for Instinct {
    fn name(&self) -> &'static str {
        "Instinct"
    }

    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        card_type == CardType::Attack
    }

    fn enchant_damage_multiplicative(
        &self,
        _enchantment: Enchantment,
        _damage: Decimal,
        props: ValueProps,
    ) -> Decimal {
        if props.is_powered_attack() {
            Decimal::from(2)
        } else {
            Decimal::ONE
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for Instinct {
    type Me = CardId;
}
