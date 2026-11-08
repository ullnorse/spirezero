//! `ChildOfTheStarsPower` (`Models.Powers/ChildOfTheStarsPower.cs`): a buff (`Counter`); when a
//! play spends stars, the owner gains `Amount` unpowered block per star spent.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct ChildOfTheStars;

impl PowerModel for ChildOfTheStars {
    fn name(&self) -> &'static str {
        "Child of the Stars"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for ChildOfTheStars {
    type Me = PowerRef;

    fn after_stars_spent(&self, eng: &mut Engine<'_>, me: PowerRef, amount: i32) -> Flow {
        // `spender == Owner.Player`: the only player.
        if amount > 0 {
            let block = eng.s.power(me.owner, me.id).amount * amount;
            eng.gain_block(me.owner, Decimal::from(block), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
