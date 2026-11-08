//! `CultistsNormal` (`Models.Encounters/CultistsNormal.cs`): a `CalcifiedCultist` and a
//! `DampCultist`.

use super::EncounterModel;
use crate::content::monsters::{CalcifiedCultist, DampCultist, MonsterKind};
use crate::rng::Rng;

/// `CultistsNormal`: a `CalcifiedCultist` and a `DampCultist`.
pub struct CultistsNormal;

impl EncounterModel for CultistsNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::CalcifiedCultist(CalcifiedCultist),
            MonsterKind::DampCultist(DampCultist),
        ]
    }
}
