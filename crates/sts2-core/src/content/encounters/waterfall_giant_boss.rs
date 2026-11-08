//! `WaterfallGiantBoss` (`Models.Encounters/WaterfallGiantBoss.cs`): one `WaterfallGiant`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, WaterfallGiant};
use crate::rng::Rng;

/// `WaterfallGiantBoss`: one `WaterfallGiant`.
pub struct WaterfallGiantBoss;

impl EncounterModel for WaterfallGiantBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::WaterfallGiant(WaterfallGiant::default())]
    }
}
