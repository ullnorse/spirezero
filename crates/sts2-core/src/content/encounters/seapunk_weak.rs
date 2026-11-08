//! `SeapunkWeak` (`Models.Encounters/SeapunkWeak.cs`): one `Seapunk`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Seapunk};
use crate::rng::Rng;

/// `SeapunkWeak`: one `Seapunk`.
pub struct SeapunkWeak;

impl EncounterModel for SeapunkWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Seapunk(Seapunk)]
    }
}
