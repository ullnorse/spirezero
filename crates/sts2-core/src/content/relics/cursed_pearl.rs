//! `CursedPearl` (`Models.Relics/CursedPearl.cs`): `AfterObtained` adds a `Greed` curse to the
//! deck (`CardPileCmd.AddCurseToDeck`), then gains 333 gold.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::{RunCard, RunCtx, RunError};

/// `GoldVar(333)`.
const GOLD: i32 = 333;

pub struct CursedPearl;

impl RelicModel for CursedPearl {
    fn name(&self) -> &'static str {
        "Cursed Pearl"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        ctx.state().add_card(RunCard::new("GREED"));
        ctx.state().gain_gold(Decimal::from(GOLD));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for CursedPearl {
    type Me = RelicId;
}
