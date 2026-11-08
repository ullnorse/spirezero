//! `ExoskeletonsWeak` (`Models.Encounters/ExoskeletonsWeak.cs`): three `Exoskeleton`s in the
//! `first` to `third` slots.

use super::EncounterModel;
use crate::content::monsters::{Exoskeleton, MonsterKind};
use crate::rng::Rng;

/// `ExoskeletonsWeak`: three `Exoskeleton`s.
pub struct ExoskeletonsWeak;

impl EncounterModel for ExoskeletonsWeak {
    fn slots(&self) -> &'static [&'static str] {
        &["first", "second", "third"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["first", "second", "third"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Exoskeleton(Exoskeleton); 3]
    }
}
