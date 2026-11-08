//! `PaelsFlesh` (`Models.Relics/PaelsFlesh.cs`): Ancient (Pael); from the owner's third turn on,
//! max energy is 1 higher (`ModifyMaxEnergy`). `Status` and the turn counter are display only.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `TurnNumber < 3` keeps the base max energy.
const FIRST_TURN: u32 = 3;

pub struct PaelsFlesh;

impl RelicModel for PaelsFlesh {
    fn name(&self) -> &'static str {
        "Pael's Flesh"
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsFlesh {
    type Me = RelicId;

    fn modify_max_energy(&self, s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        if s.player.turn_number < FIRST_TURN {
            return amount;
        }
        amount + ENERGY
    }
}
