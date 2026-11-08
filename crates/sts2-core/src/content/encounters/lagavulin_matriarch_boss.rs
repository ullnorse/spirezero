//! `LagavulinMatriarchBoss` (`Models.Encounters/LagavulinMatriarchBoss.cs`): one `LagavulinMatriarch`.

use super::EncounterModel;
use crate::content::monsters::{LagavulinMatriarch, MonsterKind};
use crate::rng::Rng;

/// `LagavulinMatriarchBoss`: a `LagavulinMatriarch`.
pub struct LagavulinMatriarchBoss;

impl EncounterModel for LagavulinMatriarchBoss {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::LagavulinMatriarch(LagavulinMatriarch)]
    }
}
