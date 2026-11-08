//! `FossilStalkerNormal` (`Models.Encounters/FossilStalkerNormal.cs`): one `FossilStalker`.

use super::EncounterModel;
use crate::content::monsters::{FossilStalker, MonsterKind};
use crate::rng::Rng;

/// `FossilStalkerNormal`: one `FossilStalker`.
pub struct FossilStalkerNormal;

impl EncounterModel for FossilStalkerNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::FossilStalker(FossilStalker)]
    }
}
