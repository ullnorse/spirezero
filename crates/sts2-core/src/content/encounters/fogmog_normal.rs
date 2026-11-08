//! `FogmogNormal` (`Models.Encounters/FogmogNormal.cs`): one `Fogmog` in the `fogmog` slot. Its
//! `EyeWithTeeth` joins later in the `illusion` slot, which `Slots` puts first.

use super::EncounterModel;
use crate::content::monsters::{Fogmog, MonsterKind};
use crate::rng::Rng;

/// `FogmogNormal`: a `Fogmog`, which summons an `EyeWithTeeth`.
pub struct FogmogNormal;

impl EncounterModel for FogmogNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["illusion", "fogmog"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["fogmog"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Fogmog(Fogmog)]
    }
}
