//! `CorpseSlugsNormal` (`Models.Encounters/CorpseSlugsNormal.cs`): three `CorpseSlug`s, whose starter
//! moves are consecutive from one `NextInt(3)` draw on the encounter's `Rng`
//! (`EnsureCorpseSlugsStartWithDifferentMoves`).

use super::EncounterModel;
use crate::content::monsters::{CorpseSlug, MonsterKind};
use crate::rng::Rng;

/// `CorpseSlugsNormal`: three `CorpseSlug`s.
pub struct CorpseSlugsNormal;

impl EncounterModel for CorpseSlugsNormal {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        CorpseSlug::with_different_moves(3, rng)
            .into_iter()
            .map(MonsterKind::CorpseSlug)
            .collect()
    }
}
