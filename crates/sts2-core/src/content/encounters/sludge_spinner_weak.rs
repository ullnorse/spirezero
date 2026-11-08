//! `SludgeSpinnerWeak` (`Models.Encounters/SludgeSpinnerWeak.cs`): one `SludgeSpinner`.

use super::EncounterModel;
use crate::content::monsters::{MonsterKind, SludgeSpinner};
use crate::rng::Rng;

/// `SludgeSpinnerWeak`: one `SludgeSpinner`.
pub struct SludgeSpinnerWeak;

impl EncounterModel for SludgeSpinnerWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::SludgeSpinner(SludgeSpinner)]
    }
}
