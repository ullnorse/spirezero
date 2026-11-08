//! `ColossusPower` (`Models.Powers/ColossusPower.cs`): the owner takes half damage from powered
//! attacks by a Vulnerable dealer; it ticks down after the enemy side's turn.

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Colossus;

/// `DynamicVar("DamageDecrease", 0.5m)`.
const DAMAGE_DECREASE: Decimal = Decimal::new(5, 1);

impl PowerModel for Colossus {
    fn name(&self) -> &'static str {
        "Colossus"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Colossus {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        match query.dealer {
            Some(dealer) if s.creature(dealer).has_power(PowerKind::Vulnerable) => DAMAGE_DECREASE,
            _ => Decimal::ONE,
        }
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side == Side::Enemy {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
