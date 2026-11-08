//! `BlurPower` (`Models.Powers/BlurPower.cs`): the owner's Block is not cleared at the start of
//! its turn; one stack is used up at every turn start of the owner's side.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

pub struct Blur;

impl PowerModel for Blur {
    fn name(&self) -> &'static str {
        "Blur"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Blur {
    type Me = PowerRef;

    fn should_clear_block(&self, _s: &CombatState, me: PowerRef, creature: CreatureId) -> bool {
        me.owner != creature
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
