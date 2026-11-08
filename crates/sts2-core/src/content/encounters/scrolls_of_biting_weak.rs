//! `ScrollsOfBitingWeak` (`Models.Encounters/ScrollsOfBitingWeak.cs`): three `ScrollOfBiting`s;
//! the first has `StarterMoveIdx = NextInt(3)` on the encounter's `Rng`, the others the next two
//! (`% 3`).

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, ScrollOfBiting};
use crate::rng::Rng;

pub struct ScrollsOfBitingWeak;

impl EncounterModel for ScrollsOfBitingWeak {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let first = rng.next_int(3);
        (0..3)
            .map(|i| {
                MonsterKind::ScrollOfBiting(ScrollOfBiting {
                    starter_move_idx: (first + i) % 3,
                })
            })
            .collect()
    }
}
