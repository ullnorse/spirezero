//! `SlumberPower` (`Models.Powers/SlumberPower.cs`): a buff (`Counter`) on a `SlumberingBeetle`.
//! Unblocked damage and the end of the owner's turn each count it down; at 0 the damage stuns the
//! owner into `WakeUpMove` (then Roll Out), while the turn end performs `WakeUpMove` at once.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::monsters::SlumberingBeetle;
use crate::content::powers::{PowerModel, PowerType};

pub struct Slumber;

impl PowerModel for Slumber {
    fn name(&self) -> &'static str {
        "Slumber"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Slumber {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner || hit.result.unblocked == 0 {
            return Ok(());
        }
        eng.decrement_power(me)?;
        if eng.s.power(me.owner, me.id).amount <= 0 {
            SlumberingBeetle::wake_up_stun(eng, me.owner);
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        eng.decrement_power(me)?;
        if eng.s.power(me.owner, me.id).amount <= 0 {
            SlumberingBeetle::wake_up_move(eng, me.owner)?;
        }
        Ok(())
    }
}
