//! `SeapunkNormal` (`Models.Encounters/SeapunkNormal.cs`): a `CalcifiedCultist` and a `Seapunk`.

use super::EncounterModel;
use crate::content::monsters::{CalcifiedCultist, MonsterKind, Seapunk};
use crate::rng::Rng;

/// `SeapunkNormal`: a `CalcifiedCultist` and a `Seapunk`.
pub struct SeapunkNormal;

impl EncounterModel for SeapunkNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::CalcifiedCultist(CalcifiedCultist),
            MonsterKind::Seapunk(Seapunk),
        ]
    }
}
