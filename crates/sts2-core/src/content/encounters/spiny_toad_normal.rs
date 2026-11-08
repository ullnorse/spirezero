//! `SpinyToadNormal` (`Models.Encounters/SpinyToadNormal.cs`): one `SpinyToad`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, SpinyToad};
use crate::rng::Rng;

/// `SpinyToadNormal`: a `SpinyToad`.
pub struct SpinyToadNormal;

impl EncounterModel for SpinyToadNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::SpinyToad(SpinyToad)]
    }
}
