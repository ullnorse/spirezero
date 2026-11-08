//! `Vigorous` (`Models.Enchantments/Vigorous.cs`): attacks only; `+Amount` to the card's powered
//! attack damage until the card is first played, which disables it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::CardType;
use crate::content::enchantments::{Enchantment, EnchantmentModel, EnchantmentStatus};
use crate::decimal::Decimal;

/// `+Amount` damage on the card's first play.
pub struct Vigorous;

impl EnchantmentModel for Vigorous {
    fn name(&self) -> &'static str {
        "Vigorous"
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
        if enchantment.status != EnchantmentStatus::Normal || !props.is_powered_attack() {
            return Decimal::ZERO;
        }
        Decimal::from(enchantment.amount)
    }
}

#[sts2_macros::hooks]
impl Hooks for Vigorous {
    type Me = CardId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        if play.card == me
            && let Some(enchantment) = eng.s.card_mut(me).enchantment.as_mut()
        {
            enchantment.status = EnchantmentStatus::Disabled;
        }
        Ok(())
    }
}
