//! `RadiancePower` (`Models.Powers/RadiancePower.cs`): after the owner's energy is reset it
//! gains 1 energy (`EnergyVar(1)`) and loses one stack.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Radiance;

/// `EnergyVar(1)`.
const ENERGY: i32 = 1;

impl PowerModel for Radiance {
    fn name(&self) -> &'static str {
        "Radiance"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Radiance {
    type Me = PowerRef;

    // `player == Owner.Player`: the only player owns it.
    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        eng.gain_energy(Decimal::from(ENERGY))?;
        eng.decrement_power(me)
    }
}
