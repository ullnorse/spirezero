//! `DenseVegetationEventEncounter` (`Models.Encounters/DenseVegetationEventEncounter.cs`): four
//! awake `Wriggler`s in the `wriggler1..4` slots. Only `DenseVegetation` starts it (it is in no
//! act's encounter list).

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Wriggler};
use crate::rng::Rng;

/// `DenseVegetationEventEncounter`: four awake `Wriggler`s (`DenseVegetation`'s fight).
pub struct DenseVegetationEventEncounter;

/// `Slots`, which `GenerateMonsters` walks in order.
const SLOTS: &[&str] = &["wriggler1", "wriggler2", "wriggler3", "wriggler4"];

impl EncounterModel for DenseVegetationEventEncounter {
    fn slots(&self) -> &'static [&'static str] {
        SLOTS
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        SLOTS
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        SLOTS
            .iter()
            .map(|_| {
                MonsterKind::Wriggler(Wriggler {
                    start_stunned: false,
                })
            })
            .collect()
    }
}
