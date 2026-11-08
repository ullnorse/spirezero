//! `TheInsatiableBoss` (`Models.Encounters/TheInsatiableBoss.cs`): one `TheInsatiable`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, TheInsatiable};
use crate::rng::Rng;

/// `TheInsatiableBoss`.
pub struct TheInsatiableBoss;

impl EncounterModel for TheInsatiableBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::TheInsatiable(TheInsatiable)]
    }
}
