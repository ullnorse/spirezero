//! `RagePower` (`Models.Powers/RagePower.cs`): `Amount` unpowered Block after each Attack the
//! owner plays; removed at the end of the owner's side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Rage;

impl PowerModel for Rage {
    fn name(&self) -> &'static str {
        "Rage"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Rage {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner == Owner.Player`: every card belongs to the player.
        if eng.s.player.creature != me.owner
            || eng.s.card(play.card).card_type() != CardType::Attack
        {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
