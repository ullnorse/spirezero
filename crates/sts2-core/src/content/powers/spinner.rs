//! `SpinnerPower` (`Models.Powers/SpinnerPower.cs`): after the owner's energy reset, channel
//! `Amount` Glass.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, OrbKind};
use crate::content::powers::{PowerModel, PowerType};

pub struct Spinner;

impl PowerModel for Spinner {
    fn name(&self) -> &'static str {
        "Spinner"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Spinner {
    type Me = PowerRef;

    // `player == Owner.Player` always holds: the only player owns the power.
    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        for _ in 0..eng.s.power(me.owner, me.id).amount {
            eng.channel(OrbKind::Glass)?;
        }
        Ok(())
    }
}
