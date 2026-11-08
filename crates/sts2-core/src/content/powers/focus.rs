//! `FocusPower` (`Models.Powers/FocusPower.cs`): `+Amount` to the owner's orb values, floored at 0.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, Orb};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Focus;

impl PowerModel for Focus {
    fn name(&self) -> &'static str {
        "Focus"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn allows_negative(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Focus {
    type Me = PowerRef;

    fn modify_orb_value(
        &self,
        s: &CombatState,
        me: PowerRef,
        _orb: Orb,
        value: Decimal,
    ) -> Decimal {
        // `Owner.Player != orb.Owner`: every orb belongs to the only player.
        if me.owner != s.player.creature {
            return value;
        }
        (value + Decimal::from(s.power(me.owner, me.id).amount)).max(Decimal::ZERO)
    }
}
