//! `KnowledgeDemonBoss` (`Models.Encounters/KnowledgeDemonBoss.cs`): one `KnowledgeDemon`.

use super::EncounterModel;
use crate::content::monsters::{KnowledgeDemon, MonsterKind};
use crate::rng::Rng;

/// `KnowledgeDemonBoss`.
pub struct KnowledgeDemonBoss;

impl EncounterModel for KnowledgeDemonBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::KnowledgeDemon(KnowledgeDemon::default())]
    }
}
