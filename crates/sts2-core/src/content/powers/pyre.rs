//! `PyrePower` (`Models.Powers/PyrePower.cs`): a buff (`Counter`); the owner's maximum energy
//! grows by `Amount`.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Pyre;

impl PowerModel for Pyre {
    fn name(&self) -> &'static str {
        "Pyre"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Pyre {
    type Me = PowerRef;

    fn modify_max_energy(&self, s: &CombatState, me: PowerRef, amount: Decimal) -> Decimal {
        // `player != Owner.Player`: the only player owns it.
        if s.player.creature != me.owner {
            return amount;
        }
        amount + Decimal::from(s.power(me.owner, me.id).amount)
    }
}
