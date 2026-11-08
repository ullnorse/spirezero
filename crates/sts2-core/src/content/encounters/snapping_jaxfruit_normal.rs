//! `SnappingJaxfruitNormal` (`Models.Encounters/SnappingJaxfruitNormal.cs`): a
//! `SnappingJaxfruit`, then a `Flyconid`.

use super::EncounterModel;
use crate::content::monsters::{Flyconid, MonsterKind, SnappingJaxfruit};
use crate::rng::Rng;

/// `SnappingJaxfruitNormal`: a `SnappingJaxfruit` and a `Flyconid`.
pub struct SnappingJaxfruitNormal;

impl EncounterModel for SnappingJaxfruitNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::SnappingJaxfruit(SnappingJaxfruit),
            MonsterKind::Flyconid(Flyconid),
        ]
    }
}
