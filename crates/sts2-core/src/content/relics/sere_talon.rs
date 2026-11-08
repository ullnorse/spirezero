//! `SereTalon` (`Models.Relics/SereTalon.cs`): Ancient (Vakuu); `AfterObtained` loses 9 max HP
//! (`CreatureCmd.LoseMaxHp`, not from a card) and adds 3 `Wish`es to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

/// `HpLossVar(9m)`.
const HP_LOSS: i32 = 9;
/// `DynamicVar("Wishes", 3m)`.
const WISHES: usize = 3;

pub struct SereTalon;

impl RelicModel for SereTalon {
    fn name(&self) -> &'static str {
        "Sere Talon"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        s.lose_max_hp(HP_LOSS);
        for _ in 0..WISHES {
            s.add_card(RunCard::new("WISH"));
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for SereTalon {
    type Me = RelicId;
}
