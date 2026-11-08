//! `RegalPillow` (`Models.Relics/RegalPillow.cs`): rest-site heals heal 15 more. Its
//! `AfterRoomEntered`, `AfterRestSiteHeal` and `ModifyExtraRestSiteHealText` only change the
//! display.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;

/// `HealVar(15m)`.
const HEAL: i32 = 15;

pub struct RegalPillow;

impl RelicModel for RegalPillow {
    fn name(&self) -> &'static str {
        "Regal Pillow"
    }

    fn modify_rest_site_heal_amount(&self, _s: &RunState, _me: usize, amount: Decimal) -> Decimal {
        amount + Decimal::from(HEAL)
    }
}

#[sts2_macros::hooks]
impl Hooks for RegalPillow {
    type Me = RelicId;
}
