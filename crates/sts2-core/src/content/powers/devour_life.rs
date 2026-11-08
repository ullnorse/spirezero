//! `DevourLifePower` (`Models.Powers/DevourLifePower.cs`): playing a `Soul` summons Osty for
//! `Amount`.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct DevourLife;

impl PowerModel for DevourLife {
    fn name(&self) -> &'static str {
        "Devour Life"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for DevourLife {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner.Creature == Owner`: every card belongs to the player.
        if eng.s.card(play.card).kind != CardKind::Soul || eng.s.player.creature != me.owner {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.summon_osty(amount)
    }
}
