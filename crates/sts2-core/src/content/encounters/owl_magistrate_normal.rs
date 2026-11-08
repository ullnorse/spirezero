//! `OwlMagistrateNormal` (`Models.Encounters/OwlMagistrateNormal.cs`): one `OwlMagistrate`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, OwlMagistrate};
use crate::rng::Rng;

/// `OwlMagistrateNormal`.
pub struct OwlMagistrateNormal;

impl EncounterModel for OwlMagistrateNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::OwlMagistrate(OwlMagistrate)]
    }
}
