//! `ParryingShield` (`Models.Relics/ParryingShield.cs`): after the owner's turn, with 10 or more
//! Block, 6 damage to a random hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::rng::RunRngType;

/// `BlockVar(10m, ValueProp.Unpowered)`.
const BLOCK: i32 = 10;
/// `DamageVar(6m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 6;

pub struct ParryingShield;

impl RelicModel for ParryingShield {
    fn name(&self) -> &'static str {
        "Parrying Shield"
    }
}

#[sts2_macros::hooks]
impl Hooks for ParryingShield {
    type Me = RelicId;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if !participants.contains(&owner) || eng.s.creature(owner).block < BLOCK {
            return Ok(());
        }
        let enemies = eng.hooked().hittable_enemies();
        if let Some(&target) = eng.s.rng[RunRngType::CombatTargets].next_item(&enemies) {
            eng.damage(
                &[target],
                Decimal::from(DAMAGE),
                ValueProps::UNPOWERED,
                Some(owner),
                None,
            )?;
        }
        Ok(())
    }
}
