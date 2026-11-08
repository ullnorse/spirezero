//! `MechaKnightElite` (`Models.Encounters/MechaKnightElite.cs`): one `MechaKnight`.

use super::EncounterModel;
use crate::content::monsters::{MechaKnight, MonsterKind};
use crate::rng::Rng;

pub struct MechaKnightElite;

impl EncounterModel for MechaKnightElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::MechaKnight(MechaKnight)]
    }
}
