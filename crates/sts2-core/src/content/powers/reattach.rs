//! `ReattachPower` (`Models.Powers/ReattachPower.cs`): a `DecimillipedeSegment`'s revive (`Single`
//! buff, amount 25). A dying segment stays in combat with this power, cannot be hit while
//! reviving, and its next move becomes `DEAD_MOVE` (then `REATTACH_MOVE`, which heals it by the
//! amount) unless every other segment is dead too, which ends the fight. The fade-out of the
//! segments is animation only.
//!
//! `ShouldOwnerDeathTriggerFatal` is `AreAllOtherSegmentsDead()`: only the last segment's kill
//! triggers Fatal.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::monsters;
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

/// A `DecimillipedeSegment`'s revive.
pub struct Reattach;

/// `ReattachPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub is_reviving: bool,
}

fn is_reviving(s: &CombatState, me: PowerRef) -> bool {
    matches!(
        *s.power_data(me.id),
        PowerData::Reattach(Data { is_reviving: true })
    )
}

fn set_reviving(s: &mut CombatState, me: PowerRef, value: bool) {
    s.set_power_data(me.id, PowerData::Reattach(Data { is_reviving: value }));
}

/// `AreAllOtherSegmentsDead`: every other teammate with a `ReattachPower` is dead.
fn are_all_other_segments_dead(s: &CombatState, owner: CreatureId) -> bool {
    let side = s.creature(owner).side;
    s.creatures_on(side)
        .into_iter()
        .filter(|&c| c != owner && s.creature(c).has_power(PowerKind::Reattach))
        .all(|c| s.creature(c).is_dead())
}

/// `DoReattach`, the segment's `REATTACH_MOVE`: unless every other segment is dead, no longer
/// reviving, then heal by the amount.
pub fn do_reattach(eng: &mut Engine<'_>, owner: CreatureId) -> Flow {
    let Some(power) = eng
        .s
        .creature(owner)
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::Reattach)
        .map(|p| (p.id, p.amount))
    else {
        return Ok(());
    };
    if are_all_other_segments_dead(eng.s, owner) {
        return Ok(());
    }
    let me = PowerRef { owner, id: power.0 };
    set_reviving(eng.s, me, false);
    eng.heal(owner, Decimal::from(power.1))
}

impl PowerModel for Reattach {
    fn name(&self) -> &'static str {
        "Reattach"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn should_be_removed_after_owner_death(&self) -> bool {
        false
    }

    /// `AreAllOtherSegmentsDead()` (`ReattachPower.cs:106-109`).
    fn should_owner_death_trigger_fatal(&self, s: &CombatState, me: PowerRef) -> bool {
        are_all_other_segments_dead(s, me.owner)
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Reattach(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Reattach {
    type Me = PowerRef;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if was_removal_prevented || me.owner != creature {
            return Ok(());
        }
        if !are_all_other_segments_dead(eng.s, me.owner) || eng.s.creature(me.owner).is_alive() {
            set_reviving(eng.s, me, true);
            let is_segment = eng
                .s
                .creature(me.owner)
                .monster
                .as_ref()
                .is_some_and(|m| monsters::decimillipede_segment::is_segment(m.kind));
            if is_segment {
                monsters::decimillipede_segment::set_dead_state(eng, me.owner);
            }
        }
        Ok(())
    }

    fn should_allow_hitting(&self, s: &CombatState, me: PowerRef, creature: CreatureId) -> bool {
        creature != me.owner || !is_reviving(s, me)
    }

    fn should_creature_be_removed_after_death(
        &self,
        _s: &CombatState,
        me: PowerRef,
        creature: CreatureId,
    ) -> bool {
        creature != me.owner
    }
}
