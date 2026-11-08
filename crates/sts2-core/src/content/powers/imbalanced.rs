//! `ImbalancedPower` (`Models.Powers/ImbalancedPower.cs`): a debuff (`Single`). When a hit the
//! owner dealt is fully blocked, a `BowlbugRock` is marked off balance (its Headbutt then stuns
//! it); any other owner is stunned into the move it last used. `Flash` is cosmetic.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::monsters::{self, BowlbugRock, MonsterKind};
use crate::content::powers::{PowerModel, PowerType};

pub struct Imbalanced;

impl PowerModel for Imbalanced {
    fn name(&self) -> &'static str {
        "Imbalanced"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Imbalanced {
    type Me = PowerRef;

    fn after_damage_given(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.dealer != Some(me.owner) || !hit.result.was_fully_blocked {
            return Ok(());
        }
        match eng.s.creature(me.owner).monster.as_ref().map(|m| m.kind) {
            Some(MonsterKind::BowlbugRock(_)) => BowlbugRock::set_off_balance(eng, me.owner),
            _ => monsters::stun(eng, me.owner, None),
        }
        Ok(())
    }
}
