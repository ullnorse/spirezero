//! `MindRotPower` (`Models.Powers/MindRotPower.cs`): a debuff (`Counter`) from the
//! `KnowledgeDemon`'s Curse of Knowledge: its owner's player draws `Amount` fewer cards per hand
//! draw (never below 0). `AfterModifyingHandDraw` only flashes.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

/// Fewer cards per hand draw.
pub struct MindRot;

impl PowerModel for MindRot {
    fn name(&self) -> &'static str {
        "Mind Rot"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for MindRot {
    type Me = PowerRef;

    /// `player != Owner.Player`: the only player is the owner's unless the owner is a pet.
    fn modify_hand_draw(&self, s: &CombatState, me: PowerRef, count: Decimal) -> Decimal {
        if !s.creature(me.owner).is_player() {
            return count;
        }
        (count - Decimal::from(s.power(me.owner, me.id).amount)).max(Decimal::ZERO)
    }
}
