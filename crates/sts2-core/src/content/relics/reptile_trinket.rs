//! `ReptileTrinket` (`Models.Relics/ReptileTrinket.cs`): using a potion in combat gives 3
//! Strength until the end of the turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, Potion, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(3m)`.
const STRENGTH: i32 = 3;

pub struct ReptileTrinket;

impl RelicModel for ReptileTrinket {
    fn name(&self) -> &'static str {
        "Reptile Trinket"
    }
}

#[sts2_macros::hooks]
impl Hooks for ReptileTrinket {
    type Me = RelicId;

    fn after_potion_used(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _potion: Potion,
        _target: Option<CreatureId>,
    ) -> Flow {
        if eng.s.in_progress {
            let owner = eng.s.player.creature;
            eng.apply_power(
                owner,
                PowerKind::ReptileTrinket,
                Decimal::from(STRENGTH),
                Some(owner),
            )?;
        }
        Ok(())
    }
}
