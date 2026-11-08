//! `KnightsElite` (`Models.Encounters/KnightsElite.cs`): a `FlailKnight`, a `SpectralKnight` and
//! a `MagiKnight` in the `first`, `second` and `third` slots;
//! `Slots` is not overridden (empty).

use super::EncounterModel;
use crate::content::monsters::{FlailKnight, MagiKnight, MonsterKind, SpectralKnight};
use crate::rng::Rng;

pub struct KnightsElite;

impl EncounterModel for KnightsElite {
    fn spawn_slots(&self) -> &'static [&'static str] {
        &["first", "second", "third"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::FlailKnight(FlailKnight),
            MonsterKind::SpectralKnight(SpectralKnight),
            MonsterKind::MagiKnight(MagiKnight),
        ]
    }
}
