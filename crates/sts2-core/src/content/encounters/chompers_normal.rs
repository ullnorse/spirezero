//! `ChompersNormal` (`Models.Encounters/ChompersNormal.cs`): two `Chomper`s, the second with
//! `ScreamFirst`.

use super::EncounterModel;
use crate::content::monsters::{Chomper, MonsterKind};
use crate::rng::Rng;

/// `ChompersNormal`: two `Chomper`s.
pub struct ChompersNormal;

impl EncounterModel for ChompersNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::Chomper(Chomper {
                scream_first: false,
            }),
            MonsterKind::Chomper(Chomper { scream_first: true }),
        ]
    }
}
