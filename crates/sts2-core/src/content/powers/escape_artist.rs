//! `EscapeArtistPower` (`Models.Powers/EscapeArtistPower.cs`): a visual timer for when the
//! `ThievingHopper` escapes (`Counter` buff). At the end of each turn of its owner's side it
//! ticks down, but never below 1 (`StartPulsing` at 1 is UI only).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

/// The `ThievingHopper`'s escape timer.
pub struct EscapeArtist;

impl PowerModel for EscapeArtist {
    fn name(&self) -> &'static str {
        "Escape Artist"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for EscapeArtist {
    type Me = PowerRef;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) && eng.s.power(me.owner, me.id).amount > 1 {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
