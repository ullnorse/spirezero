//! `HauntedShipNormal` (`Models.Encounters/HauntedShipNormal.cs`): one `HauntedShip`.

use super::EncounterModel;
use crate::content::monsters::{HauntedShip, MonsterKind};
use crate::rng::Rng;

/// `HauntedShipNormal`: one `HauntedShip`.
pub struct HauntedShipNormal;

impl EncounterModel for HauntedShipNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::HauntedShip(HauntedShip)]
    }
}
