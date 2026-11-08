//! `InfestedPrismsElite` (`Models.Encounters/InfestedPrismsElite.cs`): one `InfestedPrism`.

use super::EncounterModel;
use crate::content::monsters::{InfestedPrism, MonsterKind};
use crate::rng::Rng;

/// `InfestedPrismsElite`: one `InfestedPrism`.
pub struct InfestedPrismsElite;

impl EncounterModel for InfestedPrismsElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::InfestedPrism(InfestedPrism)]
    }
}
