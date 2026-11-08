//! `AxebotsNormal` (`Models.Encounters/AxebotsNormal.cs`): one `Axebot` in the `front` slot (its respawns take the same slot).

use super::EncounterModel;
use crate::content::monsters::{Axebot, MonsterKind};
use crate::rng::Rng;

/// `AxebotsNormal`.
pub struct AxebotsNormal;

impl EncounterModel for AxebotsNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["front"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["front"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Axebot(Axebot::default())]
    }
}
