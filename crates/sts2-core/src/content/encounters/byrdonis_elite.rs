//! `ByrdonisElite` (`Models.Encounters/ByrdonisElite.cs`): one `Byrdonis`.

use super::EncounterModel;
use crate::content::monsters::{Byrdonis, MonsterKind};
use crate::rng::Rng;

/// `ByrdonisElite`.
pub struct ByrdonisElite;

impl EncounterModel for ByrdonisElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Byrdonis(Byrdonis)]
    }
}
