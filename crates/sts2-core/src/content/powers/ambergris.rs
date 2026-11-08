//! `AmbergrisPower` (`Models.Powers/AmbergrisPower.cs`): an extra player turn for each stack,
//! consumed as the turn starts. Hidden. The combat's `SwitchFromPlayerToEnemySide` asks
//! `ShouldTakeExtraTurn` (`combat/turn.rs`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};

pub struct Ambergris;

impl PowerModel for Ambergris {
    fn name(&self) -> &'static str {
        "Ambergris"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn is_visible(&self) -> bool {
        false
    }
}

#[sts2_macros::hooks]
impl Hooks for Ambergris {
    type Me = PowerRef;

    fn should_take_extra_turn(&self, s: &CombatState, me: PowerRef) -> bool {
        // `player == Owner.Player`: the owner is the (only) player creature.
        s.power(me.owner, me.id).amount > 0 && s.creature(me.owner).is_player()
    }

    fn after_taking_extra_turn(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        if eng.s.creature(me.owner).is_player() {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
