//! `KaiserCrabBoss` (`Models.Encounters/KaiserCrabBoss.cs`): a `Crusher` in the `crusher` slot and
//! a `Rocket` in the `rocket` slot.

use super::EncounterModel;
use crate::content::monsters::{Crusher, MonsterKind, Rocket};
use crate::rng::Rng;

/// `KaiserCrabBoss`: the two crab arms.
pub struct KaiserCrabBoss;

impl EncounterModel for KaiserCrabBoss {
    fn slots(&self) -> &'static [&'static str] {
        &["crusher", "rocket"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["crusher", "rocket"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Crusher(Crusher), MonsterKind::Rocket(Rocket)]
    }
}
