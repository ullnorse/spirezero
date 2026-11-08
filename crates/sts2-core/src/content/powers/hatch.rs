//! `HatchPower` (`Models.Powers/HatchPower.cs`): a `ToughEgg`'s countdown to hatching (`Counter`
//! buff). It ticks down by one at the end of each turn of its owner's side; the egg's
//! `HATCH_MOVE` removes it.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

/// A `ToughEgg`'s countdown.
pub struct Hatch;

impl PowerModel for Hatch {
    fn name(&self) -> &'static str {
        "Hatch"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Hatch {
    type Me = PowerRef;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
