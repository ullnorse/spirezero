//! `TwoTailedRatsNormal` (`Models.Encounters/TwoTailedRatsNormal.cs`): three `TwoTailedRat`s in the
//! `third`, `fourth` and `fifth` of five slots; the first has `StarterMoveIndex = NextInt(3)` on the
//! encounter's `Rng`, the others the next two (`% 3`). Call for Backup fills the last free slot.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, TwoTailedRat};
use crate::rng::Rng;

/// `TwoTailedRatsNormal`: three `TwoTailedRat`s.
pub struct TwoTailedRatsNormal;

impl EncounterModel for TwoTailedRatsNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["first", "second", "third", "fourth", "fifth"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["third", "fourth", "fifth"]
    }

    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let first = rng.next_int(3);
        (0..3)
            .map(|i| MonsterKind::TwoTailedRat(TwoTailedRat::with_starter((first + i) % 3)))
            .collect()
    }
}
