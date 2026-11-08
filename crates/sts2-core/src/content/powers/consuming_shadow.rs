//! `ConsumingShadowPower` (`Models.Powers/ConsumingShadowPower.cs`): after the end of the owner's
//! side's turn (after the orb passives), if the queue is not empty, evoke the last orb `Amount`
//! times (each evoke dequeues it).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

pub struct ConsumingShadow;

impl PowerModel for ConsumingShadow {
    fn name(&self) -> &'static str {
        "Consuming Shadow"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for ConsumingShadow {
    type Me = PowerRef;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) && !eng.s.player.orbs.is_empty() {
            for _ in 0..eng.s.power(me.owner, me.id).amount {
                eng.evoke_last(true)?;
            }
        }
        Ok(())
    }
}
