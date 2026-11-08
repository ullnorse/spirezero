//! `Glam` (`Models.Enchantments/Glam.cs`): the card is played once more (`EnchantPlayCount`
//! +1) until its first play in a combat, which disables it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::enchantments::{Enchantment, EnchantmentModel, EnchantmentStatus};

/// `DynamicVar("Times", 1m)`.
const TIMES: u32 = 1;

/// Played once more until its first play in a combat.
pub struct Glam;

impl EnchantmentModel for Glam {
    fn name(&self) -> &'static str {
        "Glam"
    }

    fn enchant_play_count(&self, enchantment: Enchantment, original: u32) -> u32 {
        if enchantment.status == EnchantmentStatus::Normal {
            original + TIMES
        } else {
            original
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for Glam {
    type Me = CardId;

    /// `AfterCardPlayed`: the card's first play this combat sets `UsedThisCombat` and disables
    /// the enchantment.
    fn after_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        if play.card == me
            && let Some(enchantment) = eng.s.card_mut(me).enchantment.as_mut()
        {
            enchantment.status = EnchantmentStatus::Disabled;
        }
        Ok(())
    }
}
