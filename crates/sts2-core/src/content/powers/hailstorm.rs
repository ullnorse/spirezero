//! `HailstormPower` (`Models.Powers/HailstormPower.cs`): before the end of the owner's side's
//! turn (so before the orb passives), if the queue holds at least `FrostOrbs` (1) Frost orbs,
//! `Amount` unpowered damage to every hittable enemy.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, OrbKind, Side, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Hailstorm;

/// `HailstormPower.frostOrbCount`, the canonical `FrostOrbs` var.
const FROST_ORBS: usize = 1;

impl PowerModel for Hailstorm {
    fn name(&self) -> &'static str {
        "Hailstorm"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Hailstorm {
    type Me = PowerRef;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let frost = eng
                .s
                .player
                .orbs
                .iter()
                .filter(|o| o.kind == OrbKind::Frost)
                .count();
            if frost >= FROST_ORBS {
                let enemies = eng.hooked().hittable_enemies();
                let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
                eng.damage(
                    &enemies,
                    amount,
                    ValueProps::UNPOWERED,
                    Some(me.owner),
                    None,
                )?;
            }
        }
        Ok(())
    }
}
