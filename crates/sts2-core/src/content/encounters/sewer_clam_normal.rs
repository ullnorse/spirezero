//! `SewerClamNormal` (`Models.Encounters/SewerClamNormal.cs`): one `SewerClam`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, SewerClam};
use crate::rng::Rng;

/// `SewerClamNormal`: one `SewerClam`.
pub struct SewerClamNormal;

impl EncounterModel for SewerClamNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::SewerClam(SewerClam)]
    }
}
