//! `PreservedFog` (`Models.Relics/PreservedFog.cs`): Ancient (Vakuu); `AfterObtained` removes 3
//! chosen cards (`FromDeckForRemoval`), then adds a `Folly` curse (`CardPileCmd.AddCurseToDeck`)
//! once the removal screen is done.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct PreservedFog;

impl RelicModel for PreservedFog {
    fn name(&self) -> &'static str {
        "Preserved Fog"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().removable_cards();
        ctx.select(SelectPurpose::Remove { damage: 0 }, options, CARDS)?;
        ctx.then_add_card("FOLLY");
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for PreservedFog {
    type Me = RelicId;
}
