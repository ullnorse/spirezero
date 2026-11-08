//! `BufferPower` (`Models.Powers/BufferPower.cs`): the next HP loss of the owner (after the Osty
//! redirect, and after other effects have reduced it) becomes 0 and uses up one stack.
//! `GetScaledAmountForMultiplayer` is `(players - 1) * 2 + 1 = 1` times the amount here.

use crate::combat::hooks::{Hooks, HpLossQuery, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Buffer;

impl PowerModel for Buffer {
    fn name(&self) -> &'static str {
        "Buffer"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Buffer {
    type Me = PowerRef;

    fn modify_hp_lost_after_osty_late(
        &self,
        _s: &CombatState,
        me: PowerRef,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        if query.target != me.owner {
            return amount;
        }
        Decimal::ZERO
    }

    fn after_modifying_hp_lost_after_osty(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        eng.decrement_power(me)
    }
}
