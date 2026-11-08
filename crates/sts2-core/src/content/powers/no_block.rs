//! `NoBlockPower` (`Models.Powers/NoBlockPower.cs`): the owner's powered Block from cards is
//! multiplied by 0. Loses one stack after the enemy turn.

use crate::combat::hooks::{BlockQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct NoBlock;

impl PowerModel for NoBlock {
    fn name(&self) -> &'static str {
        "No Block"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for NoBlock {
    type Me = PowerRef;

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

    fn modify_block_multiplicative(
        &self,
        _s: &CombatState,
        me: PowerRef,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != me.owner
            || query.props.has(ValueProps::UNPOWERED)
            || query.card.is_none()
        {
            return Decimal::ONE;
        }
        Decimal::ZERO
    }
}
