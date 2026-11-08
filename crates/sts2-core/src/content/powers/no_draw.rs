//! `NoDrawPower` (`Models.Powers/NoDrawPower.cs`): a debuff (`Single`); the owner draws no cards
//! except for the hand draw, until the end of the turn.

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

pub struct NoDraw;

impl PowerModel for NoDraw {
    fn name(&self) -> &'static str {
        "No Draw"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for NoDraw {
    type Me = PowerRef;

    // `player != Owner.Player`: the only player owns it. `Flash` is visual only.
    fn should_draw(&self, _s: &CombatState, _me: PowerRef, from_hand_draw: bool) -> bool {
        from_hand_draw
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
