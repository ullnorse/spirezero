//! `TerrorEelElite` (`Models.Encounters/TerrorEelElite.cs`): one `TerrorEel`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, TerrorEel};
use crate::rng::Rng;

/// `TerrorEelElite`: one `TerrorEel`.
pub struct TerrorEelElite;

impl EncounterModel for TerrorEelElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::TerrorEel(TerrorEel)]
    }
}
