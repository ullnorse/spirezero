//! `TunnelerWeak` (`Models.Encounters/TunnelerWeak.cs`): one `Tunneler`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Tunneler};
use crate::rng::Rng;

/// `TunnelerWeak`: a `Tunneler`.
pub struct TunnelerWeak;

impl EncounterModel for TunnelerWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Tunneler(Tunneler)]
    }
}
