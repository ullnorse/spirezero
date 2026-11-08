//! `LightningRodPower` (`Models.Powers/LightningRodPower.cs`): after the owner's energy reset
//! (not `BeforeSideTurnStart`, so Plasma energy and Frost block of an evoked orb are not lost to
//! the reset and the block clear), channel 1 Lightning, then `Decrement` the power.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, OrbKind};
use crate::content::powers::{PowerModel, PowerType};

pub struct LightningRod;

impl PowerModel for LightningRod {
    fn name(&self) -> &'static str {
        "Lightning Rod"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for LightningRod {
    type Me = PowerRef;

    // `player == Owner.Player` always holds: the only player owns the power.
    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        eng.channel(OrbKind::Lightning)?;
        eng.decrement_power(me)
    }
}
