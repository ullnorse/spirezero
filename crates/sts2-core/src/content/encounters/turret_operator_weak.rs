//! `TurretOperatorWeak` (`Models.Encounters/TurretOperatorWeak.cs`): `LivingShield` and `TurretOperator`.

use super::EncounterModel;
use crate::content::monsters::{LivingShield, MonsterKind, TurretOperator};
use crate::rng::Rng;

/// `TurretOperatorWeak`.
pub struct TurretOperatorWeak;

impl EncounterModel for TurretOperatorWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::LivingShield(LivingShield),
            MonsterKind::TurretOperator(TurretOperator),
        ]
    }
}
