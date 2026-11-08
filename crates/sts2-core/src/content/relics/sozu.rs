//! `Sozu` (`Models.Relics/Sozu.cs`): +1 max energy; the owner can no longer procure potions
//! (`ShouldProcurePotion`, in and out of combat).

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct Sozu;

impl RelicModel for Sozu {
    fn name(&self) -> &'static str {
        "Sozu"
    }

    fn should_procure_potion_out_of_combat(&self, _s: &RunState, _me: usize) -> bool {
        // `player != Owner` is never true: the only player is the owner.
        false
    }
}

#[sts2_macros::hooks]
impl Hooks for Sozu {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        amount + ENERGY
    }

    fn should_procure_potion(&self, _s: &CombatState, _me: RelicId) -> bool {
        false
    }
}
