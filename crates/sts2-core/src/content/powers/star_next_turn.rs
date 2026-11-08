//! `StarNextTurnPower` (`Models.Powers/StarNextTurnPower.cs`): after the energy reset of the
//! owner's next turn, gain `Amount` stars and remove itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct StarNextTurn;

impl PowerModel for StarNextTurn {
    fn name(&self) -> &'static str {
        "Star Next Turn"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for StarNextTurn {
    type Me = PowerRef;

    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player owns it.
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_stars(amount)?;
        eng.remove_power(me.owner, me.id)
    }
}
