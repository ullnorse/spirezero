//! `HandDrill` (`Models.Relics/HandDrill.cs`): when the owner (or one of its pets) breaks an
//! enemy's Block, that enemy gets 2 Vulnerable (`PowerVar<VulnerablePower>(2m)`).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<VulnerablePower>(2m)`.
const VULNERABLE: i32 = 2;

pub struct HandDrill;

impl RelicModel for HandDrill {
    fn name(&self) -> &'static str {
        "Hand Drill"
    }
}

#[sts2_macros::hooks]
impl Hooks for HandDrill {
    type Me = RelicId;

    fn after_block_broken(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        target: CreatureId,
        breaker: Option<CreatureId>,
    ) -> Flow {
        let player = eng.s.player.creature;
        let Some(breaker) = breaker else {
            return Ok(());
        };
        if (breaker != player && eng.s.creature(breaker).pet_owner != Some(player))
            || target == player
        {
            return Ok(());
        }
        // Flash: visual only.
        eng.apply_power(
            target,
            PowerKind::Vulnerable,
            Decimal::from(VULNERABLE),
            Some(player),
        )
    }
}
