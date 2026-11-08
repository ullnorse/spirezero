//! `AdaptablePower` (`Models.Powers/AdaptablePower.cs`): `TestSubject`'s revive (`Single` buff).
//! The owner stays in combat when it dies (its power stays too) and keeps the combat from ending;
//! its death sends the `TestSubject` to its dead state (`RESPAWN_MOVE`), and until that move
//! revives it (`DoRevive`), the owner cannot be hit.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::monsters::{MonsterKind, TestSubject};
use crate::content::powers::{PowerData, PowerModel, PowerType};

/// `TestSubject`'s revive.
pub struct Adaptable;

/// `AdaptablePower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `isReviving`.
    pub is_reviving: bool,
}

fn is_reviving(s: &CombatState, me: PowerRef) -> bool {
    matches!(
        *s.power_data(me.id),
        PowerData::Adaptable(Data { is_reviving: true })
    )
}

fn set_reviving(s: &mut CombatState, me: PowerRef, value: bool) {
    s.set_power_data(me.id, PowerData::Adaptable(Data { is_reviving: value }));
}

/// `DoRevive` on the owner's `AdaptablePower`, if it has one.
pub fn do_revive(s: &mut CombatState, owner: CreatureId) {
    let power = s
        .creature(owner)
        .powers
        .iter()
        .find(|p| matches!(s.power_data(p.id), PowerData::Adaptable(_)))
        .map(|p| p.id);
    if let Some(id) = power {
        set_reviving(s, PowerRef { owner, id }, false);
    }
}

impl PowerModel for Adaptable {
    fn name(&self) -> &'static str {
        "Adaptable"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn should_be_removed_after_owner_death(&self) -> bool {
        false
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Adaptable(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Adaptable {
    type Me = PowerRef;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        let is_test_subject = matches!(
            eng.s.creature(creature).monster.as_ref().map(|m| m.kind),
            Some(MonsterKind::TestSubject(_))
        );
        if !was_removal_prevented && creature == me.owner && is_test_subject {
            set_reviving(eng.s, me, true);
            TestSubject::trigger_dead_state(eng, me.owner);
        }
        Ok(())
    }

    /// The `TestSubject` is not hit while it revives.
    fn should_allow_hitting(&self, s: &CombatState, me: PowerRef, creature: CreatureId) -> bool {
        creature != me.owner || !is_reviving(s, me)
    }

    fn should_stop_combat_from_ending(&self, _s: &CombatState, _me: PowerRef) -> bool {
        true
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
