//! `Ectoplasm` (`Models.Relics/Ectoplasm.cs`): +1 max energy; the owner gains no gold (in and out
//! of combat). Its `AfterModifyingGoldGained` only flashes.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct Ectoplasm;

impl RelicModel for Ectoplasm {
    fn name(&self) -> &'static str {
        "Ectoplasm"
    }

    fn modify_gold_gained_out_of_combat(
        &self,
        _s: &RunState,
        _me: usize,
        _amount: Decimal,
    ) -> Decimal {
        Decimal::ZERO
    }
}

#[sts2_macros::hooks]
impl Hooks for Ectoplasm {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        amount + ENERGY
    }

    fn modify_gold_gained(&self, _s: &CombatState, _me: RelicId, _amount: Decimal) -> Decimal {
        Decimal::ZERO
    }
}
