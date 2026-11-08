//! `HunterKillerNormal` (`Models.Encounters/HunterKillerNormal.cs`): one `HunterKiller`.

use super::EncounterModel;
use crate::content::monsters::{HunterKiller, MonsterKind};
use crate::rng::Rng;

/// `HunterKillerNormal`: one `HunterKiller`.
pub struct HunterKillerNormal;

impl EncounterModel for HunterKillerNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::HunterKiller(HunterKiller)]
    }
}
