//! `ThievingHopperWeak` (`Models.Encounters/ThievingHopperWeak.cs`): one `ThievingHopper`, no
//! slots. Tagged `Thieves`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, ThievingHopper};
use crate::rng::Rng;

/// `ThievingHopperWeak`: a `ThievingHopper`.
pub struct ThievingHopperWeak;

impl EncounterModel for ThievingHopperWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::ThievingHopper(ThievingHopper)]
    }
}
