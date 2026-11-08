//! `FlameBarrierPower` (`Models.Powers/FlameBarrierPower.cs`): a powered attack that damages the
//! owner deals `Amount` unpowered damage back to its dealer; removed when the opposing side's
//! turn ends.

use crate::combat::ValueProps;
use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct FlameBarrier;

impl PowerModel for FlameBarrier {
    fn name(&self) -> &'static str {
        "Flame Barrier"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for FlameBarrier {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        let Some(dealer) = hit.dealer else {
            return Ok(());
        };
        if hit.target == me.owner && hit.props.is_powered_attack() {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.damage(
                &[dealer],
                amount,
                ValueProps::UNPOWERED,
                Some(me.owner),
                None,
            )?;
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if eng.s.creature(me.owner).side != side {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
