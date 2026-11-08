//! `FabricatorNormal` (`Models.Encounters/FabricatorNormal.cs`): one `Fabricator` in the
//! `fabricator` slot of `bot1, bot2, fabricator, bot3, bot4`; its bots take `GetNextSlot`.

use super::EncounterModel;
use crate::content::monsters::{Fabricator, MonsterKind};
use crate::rng::Rng;

pub struct FabricatorNormal;

impl EncounterModel for FabricatorNormal {
    fn slots(&self) -> &'static [&'static str] {
        &["bot1", "bot2", "fabricator", "bot3", "bot4"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["fabricator"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Fabricator(Fabricator::default())]
    }
}
