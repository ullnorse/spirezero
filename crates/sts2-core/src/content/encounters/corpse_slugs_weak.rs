//! `CorpseSlugsWeak` (`Models.Encounters/CorpseSlugsWeak.cs`): two `CorpseSlug`s, whose starter
//! moves are consecutive from one `NextInt(3)` draw on the encounter's `Rng`
//! (`EnsureCorpseSlugsStartWithDifferentMoves`).

use super::EncounterModel;
use crate::content::monsters::{CorpseSlug, MonsterKind};
use crate::rng::Rng;

/// `CorpseSlugsWeak`: two `CorpseSlug`s.
pub struct CorpseSlugsWeak;

impl EncounterModel for CorpseSlugsWeak {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        CorpseSlug::with_different_moves(2, rng)
            .into_iter()
            .map(MonsterKind::CorpseSlug)
            .collect()
    }
}
