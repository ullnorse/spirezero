//! `BoneFlute` (`Models.Relics/BoneFlute.cs`): 2 Block after each of Osty's attacks.

use crate::combat::commands::Attack;
use crate::combat::commands::damage::DamageResult;
use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId, ValueProps};
use crate::content::monsters::Monster;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `BlockVar(2m, ValueProp.Unpowered)`.
const BLOCK: i32 = 2;

pub struct BoneFlute;

impl RelicModel for BoneFlute {
    fn name(&self) -> &'static str {
        "Bone Flute"
    }
}

#[sts2_macros::hooks]
impl Hooks for BoneFlute {
    type Me = RelicId;

    fn after_attack(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        attack: &Attack,
        _results: &[DamageResult],
    ) -> Flow {
        let owner = eng.s.player.creature;
        let attacker = eng.s.creature(attack.attacker);
        if !attacker.monster.as_ref().is_some_and(Monster::is_osty)
            || attacker.pet_owner != Some(owner)
        {
            return Ok(());
        }
        eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
