//! `TheLostAndForgottenNormal` (`Models.Encounters/TheLostAndForgottenNormal.cs`): `TheLost` and `TheForgotten`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, TheForgotten, TheLost};
use crate::rng::Rng;

/// `TheLostAndForgottenNormal`.
pub struct TheLostAndForgottenNormal;

impl EncounterModel for TheLostAndForgottenNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::TheLost(TheLost),
            MonsterKind::TheForgotten(TheForgotten),
        ]
    }
}
