//! `AsleepPower` (`Models.Powers/AsleepPower.cs`): a buff (`Counter`, the turns left) on
//! `LagavulinMatriarch`. HP lost wakes its owner: its `Plating` goes, it is stunned into Slash and
//! the power is removed. Otherwise the power counts down at the end of the owner's turn; before the
//! turn ends with one turn left, the `Plating` goes. `WakeUpMove` and `IsAwake` are animation only.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::monsters::{self, LagavulinMatriarch};
use crate::content::powers::{PowerKind, PowerModel, PowerType};

pub struct Asleep;

/// `PowerCmd.Remove(Owner.GetPower<PlatingPower>())` if the owner has one.
fn remove_plating(eng: &mut Engine<'_>, owner: CreatureId) -> Flow {
    let plating = eng
        .s
        .creature(owner)
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::Plating)
        .map(|p| p.id);
    plating.map_or(Ok(()), |id| eng.remove_power(owner, id))
}

impl PowerModel for Asleep {
    fn name(&self) -> &'static str {
        "Asleep"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Asleep {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner || hit.result.unblocked == 0 {
            return Ok(());
        }
        remove_plating(eng, me.owner)?;
        monsters::stun(eng, me.owner, Some(LagavulinMatriarch::SLASH));
        eng.remove_power(me.owner, me.id)
    }

    fn before_side_turn_end_very_early(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) && eng.s.power(me.owner, me.id).amount <= 1 {
            remove_plating(eng, me.owner)?;
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
        if participants.contains(&me.owner) {
            // At 0 the C# calls `WakeUpMove` (an animation and `IsAwake`).
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
