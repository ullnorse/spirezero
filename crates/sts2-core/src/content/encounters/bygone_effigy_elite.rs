//! `BygoneEffigyElite` (`Models.Encounters/BygoneEffigyElite.cs`): one `BygoneEffigy`.

use super::EncounterModel;
use crate::content::monsters::{BygoneEffigy, MonsterKind};
use crate::rng::Rng;

/// `BygoneEffigyElite`: one `BygoneEffigy`.
pub struct BygoneEffigyElite;

impl EncounterModel for BygoneEffigyElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::BygoneEffigy(BygoneEffigy)]
    }
}
