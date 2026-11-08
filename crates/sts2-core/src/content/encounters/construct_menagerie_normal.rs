//! `ConstructMenagerieNormal` (`Models.Encounters/ConstructMenagerieNormal.cs`): a
//! `PunchConstruct` and two `CubexConstruct`s, no slots.

use super::EncounterModel;
use crate::content::monsters::{CubexConstruct, MonsterKind, PunchConstruct};
use crate::rng::Rng;

pub struct ConstructMenagerieNormal;

impl EncounterModel for ConstructMenagerieNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::PunchConstruct(PunchConstruct::default()),
            MonsterKind::CubexConstruct(CubexConstruct),
            MonsterKind::CubexConstruct(CubexConstruct),
        ]
    }
}
