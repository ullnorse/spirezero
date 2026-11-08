//! `MysteriousKnightEventEncounter` (`Models.Encounters/MysteriousKnightEventEncounter.cs`): one
//! `MysteriousKnight`. Only `MysteriousKnight`'s event starts it.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, MysteriousKnight};
use crate::rng::Rng;

/// `MysteriousKnightEventEncounter`: one `MysteriousKnight`.
pub struct MysteriousKnightEventEncounter;

impl EncounterModel for MysteriousKnightEventEncounter {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::MysteriousKnight(MysteriousKnight)]
    }
}
