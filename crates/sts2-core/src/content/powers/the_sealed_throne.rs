//! `TheSealedThronePower` (`Models.Powers/TheSealedThronePower.cs`): a buff (`Counter`); the
//! owner gains `Amount` stars before each card it plays.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct TheSealedThrone;

impl PowerModel for TheSealedThrone {
    fn name(&self) -> &'static str {
        "The Sealed Throne"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for TheSealedThrone {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, _play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner == Owner.Player`: every card belongs to the player.
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_stars(amount)
    }
}
