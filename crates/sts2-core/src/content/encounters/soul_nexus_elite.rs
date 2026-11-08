//! `SoulNexusElite` (`Models.Encounters/SoulNexusElite.cs`): one `SoulNexus`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, SoulNexus};
use crate::rng::Rng;

pub struct SoulNexusElite;

impl EncounterModel for SoulNexusElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::SoulNexus(SoulNexus)]
    }
}
