//! `PhantasmalGardenersElite` (`Models.Encounters/PhantasmalGardenersElite.cs`): four
//! `PhantasmalGardener`s in the slots `first`..`fourth`, which `Slots` lists in that order.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, PhantasmalGardener};
use crate::rng::Rng;

/// `PhantasmalGardenersElite`: four `PhantasmalGardener`s.
pub struct PhantasmalGardenersElite;

/// `Slots`, which `GenerateMonsters` walks in order.
const SLOTS: &[&str] = &["first", "second", "third", "fourth"];

impl EncounterModel for PhantasmalGardenersElite {
    fn slots(&self) -> &'static [&'static str] {
        SLOTS
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        SLOTS
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        SLOTS
            .iter()
            .map(|_| MonsterKind::PhantasmalGardener(PhantasmalGardener))
            .collect()
    }
}
