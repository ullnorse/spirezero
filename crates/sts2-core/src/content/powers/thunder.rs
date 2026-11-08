//! `ThunderPower` (`Models.Powers/ThunderPower.cs`): after one of the owner's Lightning orbs is
//! evoked, `Amount` unpowered damage to each of the creatures it affected that is still alive.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Orb, OrbKind, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Thunder;

impl PowerModel for Thunder {
    fn name(&self) -> &'static str {
        "Thunder"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Thunder {
    type Me = PowerRef;

    // `orb.Owner == Owner.Player` always holds: the only player owns every orb.
    fn after_orb_evoked(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        orb: Orb,
        targets: &[CreatureId],
    ) -> Flow {
        if orb.kind != OrbKind::Lightning {
            return Ok(());
        }
        let living: Vec<CreatureId> = targets
            .iter()
            .copied()
            .filter(|&c| eng.s.creature(c).is_alive())
            .collect();
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.damage(&living, amount, ValueProps::UNPOWERED, Some(me.owner), None)?;
        Ok(())
    }
}
