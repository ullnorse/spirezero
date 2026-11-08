//! `LoopPower` (`Models.Powers/LoopPower.cs`): at the start of the owner's turn (after the draw),
//! trigger the raw passive of the front orb `Amount` times (`OrbCmd.Passive` without
//! `countAffectedByHooks`, so `ModifyOrbPassiveTriggerCount` does not apply). The file is
//! `loop_.rs` because `loop` is a Rust keyword.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};

pub struct Loop;

impl PowerModel for Loop {
    fn name(&self) -> &'static str {
        "Loop"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Loop {
    type Me = PowerRef;

    // `player == Owner.Player` always holds: the only player owns the power.
    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        if eng.s.player.orbs.is_empty() {
            return Ok(());
        }
        for _ in 0..eng.s.power(me.owner, me.id).amount {
            // `Orbs[0]` is read again each time: a passive can change the queue.
            let Some(&front) = eng.s.player.orbs.first() else {
                break;
            };
            eng.orb_passive(front, None, false)?;
        }
        Ok(())
    }
}
