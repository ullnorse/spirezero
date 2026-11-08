//! `ScrollsOfBitingNormal` (`Models.Encounters/ScrollsOfBitingNormal.cs`): four
//! `ScrollOfBiting`s; the first three as in `ScrollsOfBitingWeak` (`NextInt(3)` on the
//! encounter's `Rng`, then the next two), the fourth always starts at More Teeth (index 2).

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, ScrollOfBiting};
use crate::rng::Rng;

pub struct ScrollsOfBitingNormal;

impl EncounterModel for ScrollsOfBitingNormal {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let first = rng.next_int(3);
        [first, (first + 1) % 3, (first + 2) % 3, 2]
            .into_iter()
            .map(|starter_move_idx| {
                MonsterKind::ScrollOfBiting(ScrollOfBiting { starter_move_idx })
            })
            .collect()
    }
}
