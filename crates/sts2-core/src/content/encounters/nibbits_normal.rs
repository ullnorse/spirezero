//! `NibbitsNormal` (`Models.Encounters/NibbitsNormal.cs`): two `Nibbit`s in the slots `front`
//! (`IsFront` set) and `back`. `Slots` is `[front, back]`, so `SortEnemiesBySlotName` keeps the
//! spawn order.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Nibbit};
use crate::rng::Rng;

/// `NibbitsNormal`: a front and a back `Nibbit`.
pub struct NibbitsNormal;

impl EncounterModel for NibbitsNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["front", "back"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["front", "back"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::Nibbit(Nibbit {
                is_front: true,
                is_alone: false,
            }),
            MonsterKind::Nibbit(Nibbit {
                is_front: false,
                is_alone: false,
            }),
        ]
    }
}
