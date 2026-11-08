//! `CubexConstructNormal` (`Models.Encounters/CubexConstructNormal.cs`): one `CubexConstruct`.

use super::EncounterModel;
use crate::content::monsters::{CubexConstruct, MonsterKind};
use crate::rng::Rng;

/// `CubexConstructNormal`: one `CubexConstruct`.
pub struct CubexConstructNormal;

impl EncounterModel for CubexConstructNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::CubexConstruct(CubexConstruct)]
    }
}
