//! `Swift` (`Models.Enchantments/Swift.cs`): on the card's first play, draws `Amount` cards;
//! that disables it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::enchantments::{EnchantmentModel, EnchantmentStatus};
use crate::decimal::Decimal;

/// Draws `Amount` cards on the card's first play.
pub struct Swift;

impl EnchantmentModel for Swift {
    fn name(&self) -> &'static str {
        "Swift"
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
        eng.draw(Decimal::from(amount), false)?;
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Swift {
    type Me = CardId;
}
