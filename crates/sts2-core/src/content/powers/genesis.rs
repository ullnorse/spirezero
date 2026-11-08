//! `GenesisPower` (`Models.Powers/GenesisPower.cs`): a buff (`Counter`); the owner gains
//! `Amount` stars after each of its energy resets.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Genesis;

impl PowerModel for Genesis {
    fn name(&self) -> &'static str {
        "Genesis"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Genesis {
    type Me = PowerRef;

    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player owns it.
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_stars(amount)
    }
}
