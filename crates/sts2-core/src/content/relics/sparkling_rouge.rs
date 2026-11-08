//! `SparklingRouge` (`Models.Relics/SparklingRouge.cs`): 1 Strength and 1 Dexterity when the
//! owner's Block is cleared at the start of turn 3.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: i32 = 1;
/// `PowerVar<DexterityPower>(1m)`.
const DEXTERITY: i32 = 1;

pub struct SparklingRouge;

impl RelicModel for SparklingRouge {
    fn name(&self) -> &'static str {
        "Sparkling Rouge"
    }
}

#[sts2_macros::hooks]
impl Hooks for SparklingRouge {
    type Me = RelicId;

    fn after_block_cleared(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        creature: CreatureId,
    ) -> Flow {
        let owner = eng.s.player.creature;
        if creature == owner && eng.s.player.turn_number == 3 {
            eng.apply_power(
                owner,
                PowerKind::Strength,
                Decimal::from(STRENGTH),
                Some(owner),
            )?;
            eng.apply_power(
                owner,
                PowerKind::Dexterity,
                Decimal::from(DEXTERITY),
                Some(owner),
            )?;
        }
        Ok(())
    }
}
