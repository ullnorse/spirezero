//! `ExoskeletonsNormal` (`Models.Encounters/ExoskeletonsNormal.cs`): four `Exoskeleton`s in the
//! `first` to `fourth` slots.

use super::EncounterModel;
use crate::content::monsters::{Exoskeleton, MonsterKind};
use crate::rng::Rng;

/// `ExoskeletonsNormal`: four `Exoskeleton`s.
pub struct ExoskeletonsNormal;

impl EncounterModel for ExoskeletonsNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["first", "second", "third", "fourth"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["first", "second", "third", "fourth"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Exoskeleton(Exoskeleton); 4]
    }
}
