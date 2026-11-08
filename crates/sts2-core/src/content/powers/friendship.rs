//! `FriendshipPower` (`Models.Powers/FriendshipPower.cs`): `Amount` more max energy.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Friendship;

impl PowerModel for Friendship {
    fn name(&self) -> &'static str {
        "Friendship"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Friendship {
    type Me = PowerRef;

    fn modify_max_energy(&self, s: &CombatState, me: PowerRef, amount: Decimal) -> Decimal {
        amount + Decimal::from(s.power(me.owner, me.id).amount)
    }
}
