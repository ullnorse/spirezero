//! `NeowsTalisman` (`Models.Relics/NeowsTalisman.cs`): `AfterObtained` upgrades the deck's last
//! Basic Strike and its last Basic Defend.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

pub struct NeowsTalisman;

impl RelicModel for NeowsTalisman {
    fn name(&self) -> &'static str {
        "Neow's Talisman"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let strike = s.deck.iter().rposition(|c| c.is_basic() && c.is_strike());
        let defend = s.deck.iter().rposition(|c| c.is_basic() && c.is_defend());
        for i in [strike, defend].into_iter().flatten() {
            s.deck[i].upgrade();
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for NeowsTalisman {
    type Me = RelicId;
}
