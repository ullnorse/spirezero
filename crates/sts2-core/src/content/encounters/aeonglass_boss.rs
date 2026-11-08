//! `AeonglassBoss` (`Models.Encounters/AeonglassBoss.cs`): one `Aeonglass`.

use super::EncounterModel;
use crate::content::monsters::{Aeonglass, MonsterKind};
use crate::rng::Rng;

/// `AeonglassBoss`.
pub struct AeonglassBoss;

impl EncounterModel for AeonglassBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::Aeonglass(Aeonglass::default())]
    }
}
