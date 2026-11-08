//! `NibbitsWeak` (`Models.Encounters/NibbitsWeak.cs`): one `Nibbit` with `IsAlone` set.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Nibbit};
use crate::rng::Rng;

/// `NibbitsWeak`: one `Nibbit`, alone.
pub struct NibbitsWeak;

impl EncounterModel for NibbitsWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Nibbit(Nibbit {
            is_front: false,
            is_alone: true,
        })]
    }
}
