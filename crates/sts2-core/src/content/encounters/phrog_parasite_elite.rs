//! `PhrogParasiteElite` (`Models.Encounters/PhrogParasiteElite.cs`): one `PhrogParasite` in the
//! `phrog` slot. Its `Wriggler`s join later in the `wriggler<n>` slots.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, PhrogParasite};
use crate::rng::Rng;

/// `PhrogParasiteElite`.
pub struct PhrogParasiteElite;

impl EncounterModel for PhrogParasiteElite {
    fn slots(&self) -> &'static [&'static str] {
        &["phrog", "wriggler1", "wriggler2", "wriggler3", "wriggler4"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["phrog"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::PhrogParasite(PhrogParasite)]
    }
}
