//! `MawlerNormal` (`Models.Encounters/MawlerNormal.cs`): one `Mawler`.

use super::EncounterModel;
use crate::content::monsters::{Mawler, MonsterKind};
use crate::rng::Rng;

/// `MawlerNormal`: one `Mawler`.
pub struct MawlerNormal;

impl EncounterModel for MawlerNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Mawler(Mawler)]
    }
}
