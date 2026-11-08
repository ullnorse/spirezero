//! `EntomancerElite` (`Models.Encounters/EntomancerElite.cs`): one `Entomancer`.

use super::EncounterModel;
use crate::content::monsters::{Entomancer, MonsterKind};
use crate::rng::Rng;

/// `EntomancerElite`: one `Entomancer`.
pub struct EntomancerElite;

impl EncounterModel for EntomancerElite {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Entomancer(Entomancer)]
    }
}
