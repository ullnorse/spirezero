//! `BurrowedPower` (`Models.Powers/BurrowedPower.cs`): a buff (`Single`) on a `Tunneler`. Its
//! owner keeps its block over turns; when that block is broken the owner is stunned into Bite and
//! the power is removed, and removing it takes any block left. `GetStunned` is an animation.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::monsters::{self, MonsterKind, Tunneler};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Burrowed;

impl PowerModel for Burrowed {
    fn name(&self) -> &'static str {
        "Burrowed"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    /// `LoseBlock(oldOwner, 999999999m, null)`.
    fn after_removed(&self, eng: &mut Engine<'_>, _power: &Power, old_owner: CreatureId) -> Flow {
        eng.lose_block(old_owner, Decimal::from(999_999_999), None)
    }
}

#[sts2_macros::hooks]
impl Hooks for Burrowed {
    type Me = PowerRef;

    fn should_clear_block(&self, _s: &CombatState, me: PowerRef, creature: CreatureId) -> bool {
        me.owner != creature
    }

    fn after_block_broken(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        target: CreatureId,
        _breaker: Option<CreatureId>,
    ) -> Flow {
        if target != me.owner {
            return Ok(());
        }
        if let Some(MonsterKind::Tunneler(_)) =
            eng.s.creature(target).monster.as_ref().map(|m| m.kind)
        {
            monsters::stun(eng, me.owner, Some(Tunneler::BITE));
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
