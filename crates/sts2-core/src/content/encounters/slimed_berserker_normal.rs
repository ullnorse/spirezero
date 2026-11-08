//! `SlimedBerserkerNormal` (`Models.Encounters/SlimedBerserkerNormal.cs`): one `SlimedBerserker`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, SlimedBerserker};
use crate::rng::Rng;

/// `SlimedBerserkerNormal`.
pub struct SlimedBerserkerNormal;

impl EncounterModel for SlimedBerserkerNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::SlimedBerserker(SlimedBerserker)]
    }
}
