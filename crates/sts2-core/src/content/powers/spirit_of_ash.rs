//! `SpiritOfAshPower` (`Models.Powers/SpiritOfAshPower.cs`): before the owner plays an Ethereal
//! card, gain `Amount` unpowered Block.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::CardKeyword;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SpiritOfAsh;

impl PowerModel for SpiritOfAsh {
    fn name(&self) -> &'static str {
        "Spirit of Ash"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SpiritOfAsh {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner == Owner.Player`: every card belongs to the player.
        if eng.s.player.creature != me.owner
            || !eng.hooked().has_keyword(play.card, CardKeyword::Ethereal)
        {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
