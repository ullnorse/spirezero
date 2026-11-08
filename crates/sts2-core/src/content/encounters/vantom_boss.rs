//! `VantomBoss` (`Models.Encounters/VantomBoss.cs`): one `Vantom`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, Vantom};
use crate::rng::Rng;

/// `VantomBoss`.
pub struct VantomBoss;

impl EncounterModel for VantomBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Vantom(Vantom)]
    }
}
