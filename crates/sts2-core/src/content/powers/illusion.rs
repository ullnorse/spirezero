//! `IllusionPower` (`Models.Powers/IllusionPower.cs`): an illusion that revives. Applying it
//! also applies `MinionPower`. When its owner dies it stays in combat (and, while any illusion
//! is in combat, every dying creature keeps its buffs), cannot be hit, and its next move becomes
//! `REVIVE_MOVE`, which heals it to full and then returns to the move it last rolled. A buff
//! (`StackType` `Single`).
//!
//! `FollowUpStateId` is never set by an in-scope monster, so the revive always follows up with
//! the last logged move. `AfterCombatEnd` only plays an animation.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::monsters::machine::ForcedMove;
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

/// `EyeWithTeeth`'s revive.
pub struct Illusion;

/// `IllusionPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub is_reviving: bool,
}

/// `IsReviving`.
fn is_reviving(s: &CombatState, me: PowerRef) -> bool {
    matches!(
        *s.power_data(me.id),
        PowerData::Illusion(Data { is_reviving: true })
    )
}

fn set_reviving(s: &mut CombatState, me: PowerRef, value: bool) {
    s.set_power_data(me.id, PowerData::Illusion(Data { is_reviving: value }));
}

impl PowerModel for Illusion {
    fn name(&self) -> &'static str {
        "Illusion"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Illusion(Data::default())
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        if eng.s.creature(me.owner).has_power(PowerKind::Minion) {
            return Ok(());
        }
        eng.apply_power(me.owner, PowerKind::Minion, Decimal::ONE, None)
    }
}

#[sts2_macros::hooks]
impl Hooks for Illusion {
    type Me = PowerRef;

    /// Any dying creature keeps every power but its debuffs, except the `ITemporaryPower` ones
    /// (`EnfeeblingTouch`), which stay with the Strength they took.
    fn should_power_be_removed_on_death(
        &self,
        s: &CombatState,
        _me: PowerRef,
        power: PowerRef,
    ) -> bool {
        let kind = s.power(power.owner, power.id).kind;
        kind.power_type() == PowerType::Debuff && !kind.is_temporary()
    }

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if !was_removal_prevented && creature == me.owner {
            set_reviving(eng.s, me, true);
            if let Some(m) = eng.s.creature_mut(me.owner).monster.as_mut() {
                let follow_up = m.machine.log.last().expect("a monster has logged a move");
                m.machine
                    .set_move_immediate(ForcedMove::IllusionRevive, follow_up, true);
            }
        }
        Ok(())
    }

    /// The owner receives no powers and cannot be hit while reviving.
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

/// `ReviveMove` of the owner's Illusion: no longer reviving, then heal to full.
pub fn revive_move(eng: &mut Engine<'_>, owner: CreatureId) -> Flow {
    let power = eng
        .s
        .creature(owner)
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::Illusion)
        .map(|p| p.id)
        .expect("a reviving creature has its Illusion");
    set_reviving(eng.s, PowerRef { owner, id: power }, false);
    let c = eng.s.creature(owner);
    let missing = Decimal::from(c.max_hp - c.hp);
    eng.heal(owner, missing)
}
