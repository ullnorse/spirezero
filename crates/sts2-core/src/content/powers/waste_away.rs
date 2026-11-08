//! `WasteAwayPower` (`Models.Powers/WasteAwayPower.cs`): a debuff (`Counter`) from the
//! `KnowledgeDemon`'s Curse of Knowledge: its owner's player has `Amount` less max energy.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

/// Less max energy.
pub struct WasteAway;

impl PowerModel for WasteAway {
    fn name(&self) -> &'static str {
        "Waste Away"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for WasteAway {
    type Me = PowerRef;

    /// `player != Owner.Player`: the only player is the owner's unless the owner is a pet.
    fn modify_max_energy(&self, s: &CombatState, me: PowerRef, amount: Decimal) -> Decimal {
        if !s.creature(me.owner).is_player() {
            return amount;
        }
        amount - Decimal::from(s.power(me.owner, me.id).amount)
    }
}
