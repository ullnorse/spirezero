//! `MeatOnTheBone` (`Models.Relics/MeatOnTheBone.cs`): heals 12 after a won fight if the owner is
//! at or below half of max HP. (`Status`, set by `BeforeCombatStart` and `AfterCurrentHpChanged`,
//! only drives the relic's glow.)

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `HealVar(12m)`.
const HEAL: i32 = 12;

pub struct MeatOnTheBone;

impl RelicModel for MeatOnTheBone {
    fn name(&self) -> &'static str {
        "Meat on the Bone"
    }
}

#[sts2_macros::hooks]
impl Hooks for MeatOnTheBone {
    type Me = RelicId;

    fn after_combat_victory_early(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let owner = eng.s.creature(eng.s.player.creature);
        if owner.is_dead() {
            return Ok(());
        }
        // WillHealOnCombatFinished: CurrentHp <= (int)(MaxHp * (HpThreshold(50m) / 100m)), i.e. `* 0.5m`.
        let threshold = (Decimal::from(owner.max_hp) * Decimal::new(5, 1)).to_int_truncating();
        if owner.hp <= threshold {
            let id = owner.id;
            eng.heal(id, Decimal::from(HEAL))?;
        }
        Ok(())
    }
}
