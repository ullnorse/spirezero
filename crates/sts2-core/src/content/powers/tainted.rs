//! `TaintedPower` (`Models.Powers/TaintedPower.cs`): a debuff (`Counter`) adding `Amount` to the
//! powered attacks that hit its owner, removed when the enemies' turn ends. Played `Tainted` Skills
//! apply it (`VitalSparkPower`).

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Tainted;

impl PowerModel for Tainted {
    fn name(&self) -> &'static str {
        "Tainted"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Tainted {
    type Me = PowerRef;

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ZERO;
        }
        Decimal::from(s.power(me.owner, me.id).amount)
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side == Side::Enemy {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
