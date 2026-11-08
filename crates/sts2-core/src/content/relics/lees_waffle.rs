//! `LeesWaffle` (`Models.Relics/LeesWaffle.cs`): `AfterObtained` gains 7 max HP, then heals to
//! full.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(7m)`.
const MAX_HP: i32 = 7;

pub struct LeesWaffle;

impl RelicModel for LeesWaffle {
    fn name(&self) -> &'static str {
        "Lee's Waffle"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        s.gain_max_hp(MAX_HP);
        s.heal(Decimal::from(s.max_hp - s.hp));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for LeesWaffle {
    type Me = RelicId;
}
