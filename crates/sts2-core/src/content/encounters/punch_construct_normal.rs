//! `PunchConstructNormal` (`Models.Encounters/PunchConstructNormal.cs`): one `PunchConstruct`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, PunchConstruct};
use crate::rng::Rng;

/// `PunchConstructNormal`: one `PunchConstruct`.
pub struct PunchConstructNormal;

impl EncounterModel for PunchConstructNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::PunchConstruct(PunchConstruct::default())]
    }
}
