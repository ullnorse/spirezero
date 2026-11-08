//! `FakeLeesWaffle` (`Models.Relics/FakeLeesWaffle.cs`): `FakeMerchant`'s `LeesWaffle`;
//! `AfterObtained` heals 10% of max HP.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RunCtx, RunError};

/// `HealVar(10m)` over `100m`: the share of max HP healed.
const HEAL_FRACTION: Decimal = Decimal::new(10, 2);

pub struct FakeLeesWaffle;

impl RelicModel for FakeLeesWaffle {
    fn name(&self) -> &'static str {
        "Lee's Waffle???"
    }

    /// `Heal(creature, (decimal)MaxHp * (Heal.BaseValue / 100m))`.
    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        s.heal(Decimal::from(s.max_hp) * HEAL_FRACTION);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeLeesWaffle {
    type Me = RelicId;
}
