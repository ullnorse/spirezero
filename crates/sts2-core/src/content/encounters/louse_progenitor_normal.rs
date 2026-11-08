//! `LouseProgenitorNormal` (`Models.Encounters/LouseProgenitorNormal.cs`): one `LouseProgenitor`.

use super::EncounterModel;
use crate::content::monsters::{LouseProgenitor, MonsterKind};
use crate::rng::Rng;

/// `LouseProgenitorNormal`: a `LouseProgenitor`.
pub struct LouseProgenitorNormal;

impl EncounterModel for LouseProgenitorNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::LouseProgenitor(LouseProgenitor)]
    }
}
