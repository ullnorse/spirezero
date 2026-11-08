//! `ShrinkerBeetleWeak` (`Models.Encounters/ShrinkerBeetleWeak.cs`): one `ShrinkerBeetle`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, ShrinkerBeetle};
use crate::rng::Rng;

/// `ShrinkerBeetleWeak`: one `ShrinkerBeetle`.
pub struct ShrinkerBeetleWeak;

impl EncounterModel for ShrinkerBeetleWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::ShrinkerBeetle(ShrinkerBeetle)]
    }
}
