//! `GlobeHeadNormal` (`Models.Encounters/GlobeHeadNormal.cs`): one `GlobeHead`.

use super::EncounterModel;
use crate::content::monsters::{GlobeHead, MonsterKind};
use crate::rng::Rng;

/// `GlobeHeadNormal`.
pub struct GlobeHeadNormal;

impl EncounterModel for GlobeHeadNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::GlobeHead(GlobeHead)]
    }
}
