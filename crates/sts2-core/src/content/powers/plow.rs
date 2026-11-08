//! `PlowPower` (`Models.Powers/PlowPower.cs`): a debuff (`Counter`) on `CeremonialBeast` itself.
//! When a hit takes HP off its owner and leaves it at or below `Amount` HP, the owner loses its
//! temporary Strength and its Strength, is stunned (for a `CeremonialBeast`, into its second
//! phase: `STUNNED`, then Beast Cry), and the power is removed.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::monsters::{self, CeremonialBeast, MonsterKind};
use crate::content::powers::{PowerKind, PowerModel, PowerType};

pub struct Plow;

impl PowerModel for Plow {
    fn name(&self) -> &'static str {
        "Plow"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Plow {
    type Me = PowerRef;

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        let owner = eng.s.creature(me.owner);
        if hit.target != me.owner
            || hit.result.unblocked <= 0
            || owner.hp > owner.power_amount(PowerKind::Plow)
        {
            return Ok(());
        }
        // `GetPowerInstances<TemporaryStrengthPower>`: every subclass (Enfeebling Touch,
        // Monarch's Gaze, Dark Shackles, ...) takes or grants Strength.
        let temporary: Vec<_> = owner
            .powers
            .iter()
            .filter(|p| p.kind.internally_applied_power() == Some(PowerKind::Strength))
            .map(|p| p.id)
            .collect();
        for power in temporary {
            eng.remove_power(me.owner, power)?;
        }
        let strength = eng
            .s
            .creature(me.owner)
            .powers
            .iter()
            .find(|p| p.kind == PowerKind::Strength)
            .map(|p| p.id);
        if let Some(power) = strength {
            eng.remove_power(me.owner, power)?;
        }
        // `CeremonialBeast.SetStunned` (animation and `IsInSecondPhase`, both UI), then
        // `CreatureCmd.Stun(Owner, StunnedMove, BeastCryState.StateId)`; any other owner is
        // stunned back into the move it last used.
        let next = match eng.s.creature(me.owner).monster.as_ref().map(|m| m.kind) {
            Some(MonsterKind::CeremonialBeast(_)) => Some(CeremonialBeast::BEAST_CRY),
            _ => None,
        };
        monsters::stun(eng, me.owner, next);
        eng.remove_power(me.owner, me.id)
    }
}
