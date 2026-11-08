//! `SoulFyshBoss` (`Models.Encounters/SoulFyshBoss.cs`): one `SoulFysh`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, SoulFysh};
use crate::rng::Rng;

/// `SoulFyshBoss`: one `SoulFysh`.
pub struct SoulFyshBoss;

impl EncounterModel for SoulFyshBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::SoulFysh(SoulFysh)]
    }
}
