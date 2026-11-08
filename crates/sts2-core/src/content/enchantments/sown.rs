//! `Sown` (`Models.Enchantments/Sown.cs`): on the card's first play, `Amount` energy; that
//! disables it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::enchantments::{EnchantmentModel, EnchantmentStatus};
use crate::decimal::Decimal;

/// Gains `Amount` energy on the card's first play.
pub struct Sown;

impl EnchantmentModel for Sown {
    fn name(&self) -> &'static str {
        "Sown"
    }

    fn on_play(&self, eng: &mut Engine<'_>, card: CardId, _play: &CardPlay) -> Flow {
        let Some(enchantment) = eng.s.card_mut(card).enchantment.as_mut() else {
            return Ok(());
        };
        if enchantment.status != EnchantmentStatus::Normal {
            return Ok(());
        }
        enchantment.status = EnchantmentStatus::Disabled;
        let amount = enchantment.amount;
        eng.gain_energy(Decimal::from(amount))
    }
}

#[sts2_macros::hooks]
impl Hooks for Sown {
    type Me = CardId;
}
