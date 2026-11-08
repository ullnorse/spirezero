//! `CeremonialBeastBoss` (`Models.Encounters/CeremonialBeastBoss.cs`): one `CeremonialBeast`.

use super::EncounterModel;
use crate::content::monsters::{CeremonialBeast, MonsterKind};
use crate::rng::Rng;

/// `CeremonialBeastBoss`.
pub struct CeremonialBeastBoss;

impl EncounterModel for CeremonialBeastBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::CeremonialBeast(CeremonialBeast)]
    }
}
