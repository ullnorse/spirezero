//! `Sharp` (`Models.Enchantments/Sharp.cs`): attacks only; `+Amount` to the card's powered
//! attack damage.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, ValueProps};
use crate::content::cards::CardType;
use crate::content::enchantments::{Enchantment, EnchantmentModel};
use crate::decimal::Decimal;

/// `+Amount` damage on an attack.
pub struct Sharp;

impl EnchantmentModel for Sharp {
    fn name(&self) -> &'static str {
        "Sharp"
    }

    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        card_type == CardType::Attack
    }

    fn enchant_damage_additive(
        &self,
        enchantment: Enchantment,
        _damage: Decimal,
        props: ValueProps,
    ) -> Decimal {
        if !props.is_powered_attack() {
            return Decimal::ZERO;
        }
        Decimal::from(enchantment.amount)
    }
}

#[sts2_macros::hooks]
impl Hooks for Sharp {
    type Me = CardId;
}
