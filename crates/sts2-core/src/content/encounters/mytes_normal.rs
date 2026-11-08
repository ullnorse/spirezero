//! `MytesNormal` (`Models.Encounters/MytesNormal.cs`): two `Myte`s in the `first` and `second`
//! slots.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Myte};
use crate::rng::Rng;

/// `MytesNormal`: two `Myte`s.
pub struct MytesNormal;

impl EncounterModel for MytesNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["first", "second"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["first", "second"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Myte(Myte); 2]
    }
}
