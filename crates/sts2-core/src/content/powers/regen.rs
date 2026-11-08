//! `RegenPower` (`Models.Powers/RegenPower.cs`): before the owner's side's turn ends (early) a
//! living owner heals `Amount` and loses one stack. `ShouldScaleInMultiplayer` only matters with
//! several players.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Regen;

impl PowerModel for Regen {
    fn name(&self) -> &'static str {
        "Regen"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Regen {
    type Me = PowerRef;

    fn before_side_turn_end_early(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) && !eng.s.creature(me.owner).is_dead() {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.heal(me.owner, amount)?;
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
