//! `FrogKnightNormal` (`Models.Encounters/FrogKnightNormal.cs`): one `FrogKnight`.

use super::EncounterModel;
use crate::content::monsters::{FrogKnight, MonsterKind};
use crate::rng::Rng;

/// `FrogKnightNormal`.
pub struct FrogKnightNormal;

impl EncounterModel for FrogKnightNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::FrogKnight(FrogKnight::default())]
    }
}
