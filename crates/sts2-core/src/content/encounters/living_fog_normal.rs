//! `LivingFogNormal` (`Models.Encounters/LivingFogNormal.cs`): one `LivingFog` in the `livingFog`
//! slot, which summons `GasBomb`s into the `bomb1`..`bomb5` slots (listed first in `Slots`).

use super::EncounterModel;
use crate::content::monsters::{LivingFog, MonsterKind};
use crate::rng::Rng;

/// `LivingFogNormal`: a `LivingFog`.
pub struct LivingFogNormal;

impl EncounterModel for LivingFogNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["bomb1", "bomb2", "bomb3", "bomb4", "bomb5", "livingFog"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["livingFog"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::LivingFog(LivingFog)]
    }
}
