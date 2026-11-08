//! `BloodSoakedRose` (`Models.Relics/BloodSoakedRose.cs`): Ancient (Vakuu); +1 max energy, and
//! `AfterObtained` adds an `Enthralled` curse to the deck.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RunCard, RunCtx, RunError};

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct BloodSoakedRose;

impl RelicModel for BloodSoakedRose {
    fn name(&self) -> &'static str {
        "Blood-Soaked Rose"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().add_card(RunCard::new("ENTHRALLED"));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for BloodSoakedRose {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        // `player != Owner` is never true: the only player is the owner.
        amount + ENERGY
    }
}
