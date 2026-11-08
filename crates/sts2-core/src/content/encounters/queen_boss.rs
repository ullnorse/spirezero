//! `QueenBoss` (`Models.Encounters/QueenBoss.cs`): a `TorchHeadAmalgam` in the `amalgam` slot and
//! the `Queen` in the `queen` slot.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Queen, TorchHeadAmalgam};
use crate::rng::Rng;

/// `QueenBoss`.
pub struct QueenBoss;

impl EncounterModel for QueenBoss {
    fn slots(&self) -> &'static [&'static str] {
        &["amalgam", "queen"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["amalgam", "queen"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::TorchHeadAmalgam(TorchHeadAmalgam),
            MonsterKind::Queen(Queen::default()),
        ]
    }
}
