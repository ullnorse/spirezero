//! `FrailPower` (`Models.Powers/FrailPower.cs`): the owner gains 25% less block from powered
//! card and monster-move block; ticks down after the enemy side's turn.

use crate::combat::hooks::{BlockQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType, tick_down_duration};
use crate::decimal::Decimal;

pub struct Frail;

/// The block factor, `0.75m`.
const BLOCK_FACTOR: Decimal = Decimal::new(75, 2);

impl PowerModel for Frail {
    fn name(&self) -> &'static str {
        "Frail"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Frail {
    type Me = PowerRef;

    fn modify_block_multiplicative(
        &self,
        _s: &CombatState,
        me: PowerRef,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != me.owner || !query.props.is_powered_card_or_monster_move_block() {
            return Decimal::ONE;
        }
        BLOCK_FACTOR
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side == Side::Enemy {
            tick_down_duration(eng, me)?;
        }
        Ok(())
    }
}
