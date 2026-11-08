//! `VineShamblerNormal` (`Models.Encounters/VineShamblerNormal.cs`): one `VineShambler`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, VineShambler};
use crate::rng::Rng;

/// `VineShamblerNormal`: one `VineShambler`.
pub struct VineShamblerNormal;

impl EncounterModel for VineShamblerNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::VineShambler(VineShambler)]
    }
}
