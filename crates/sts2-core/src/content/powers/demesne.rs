//! `DemesnePower` (`Models.Powers/DemesnePower.cs`): `Amount` more cards drawn each turn and
//! `Amount` more max energy.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Demesne;

impl PowerModel for Demesne {
    fn name(&self) -> &'static str {
        "Demesne"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Demesne {
    type Me = PowerRef;

    fn modify_hand_draw(&self, s: &CombatState, me: PowerRef, count: Decimal) -> Decimal {
        count + Decimal::from(s.power(me.owner, me.id).amount)
    }

    fn modify_max_energy(&self, s: &CombatState, me: PowerRef, amount: Decimal) -> Decimal {
        amount + Decimal::from(s.power(me.owner, me.id).amount)
    }
}
