//! `Adroit` (`Models.Enchantments/Adroit.cs`): the card gains `Amount` block each time it is
//! played, after its own `OnPlay` (`Block` is a `BlockVar(0, Move)` set to `Amount`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::enchantments::EnchantmentModel;
use crate::decimal::Decimal;

/// Gains `Amount` block when played.
pub struct Adroit;

impl EnchantmentModel for Adroit {
    fn name(&self) -> &'static str {
        "Adroit"
    }

    fn on_play(&self, eng: &mut Engine<'_>, card: CardId, play: &CardPlay) -> Flow {
        let amount = eng.s.card(card).enchantment.map_or(0, |e| e.amount);
        let player = eng.s.player.creature;
        eng.gain_block(player, Decimal::from(amount), ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Adroit {
    type Me = CardId;
}
