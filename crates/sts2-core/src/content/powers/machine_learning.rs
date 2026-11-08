//! `MachineLearningPower` (`Models.Powers/MachineLearningPower.cs`): a buff (`Counter`); the
//! owner draws `Amount` more cards each turn.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct MachineLearning;

impl PowerModel for MachineLearning {
    fn name(&self) -> &'static str {
        "Machine Learning"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for MachineLearning {
    type Me = PowerRef;

    // `player != Owner.Player`: the only player owns it.
    fn modify_hand_draw(&self, s: &CombatState, me: PowerRef, count: Decimal) -> Decimal {
        count + Decimal::from(s.power(me.owner, me.id).amount)
    }
}
