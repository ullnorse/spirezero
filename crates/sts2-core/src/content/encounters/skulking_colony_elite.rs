//! `SkulkingColonyElite` (`Models.Encounters/SkulkingColonyElite.cs`): one `SkulkingColony`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, SkulkingColony};
use crate::rng::Rng;

/// `SkulkingColonyElite`: one `SkulkingColony`.
pub struct SkulkingColonyElite;

impl EncounterModel for SkulkingColonyElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::SkulkingColony(SkulkingColony)]
    }
}
