//! `DevotedSculptorWeak` (`Models.Encounters/DevotedSculptorWeak.cs`): one `DevotedSculptor`.

use super::EncounterModel;
use crate::content::monsters::{DevotedSculptor, MonsterKind};
use crate::rng::Rng;

/// `DevotedSculptorWeak`.
pub struct DevotedSculptorWeak;

impl EncounterModel for DevotedSculptorWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::DevotedSculptor(DevotedSculptor)]
    }
}
