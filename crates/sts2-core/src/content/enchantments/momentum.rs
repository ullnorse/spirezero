//! `Momentum` (`Models.Enchantments/Momentum.cs`): attacks only; each play adds `Amount` to a
//! private `_extraDamage`, which is added to the card's powered attack damage.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::CardType;
use crate::content::enchantments::{Enchantment, EnchantmentModel};
use crate::decimal::Decimal;

/// Each play adds `Amount` to the attack's damage for the rest of the combat.
pub struct Momentum;

impl EnchantmentModel for Momentum {
    fn name(&self) -> &'static str {
        "Momentum"
    }

    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        card_type == CardType::Attack
    }

    fn on_play(&self, eng: &mut Engine<'_>, card: CardId, _play: &CardPlay) -> Flow {
        if let Some(enchantment) = eng.s.card_mut(card).enchantment.as_mut() {
            enchantment.extra_damage += enchantment.amount;
        }
        Ok(())
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
        Decimal::from(enchantment.extra_damage)
    }
}

#[sts2_macros::hooks]
impl Hooks for Momentum {
    type Me = CardId;
}
