//! `PrecariousShears` (`Models.Relics/PrecariousShears.cs`): `AfterObtained` removes 2 chosen
//! cards (`FromDeckForRemoval`), then deals 16 damage to the owner (unblocked outside combat).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(2)`.
const CARDS: usize = 2;
/// `DamageVar(16m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 16;

pub struct PrecariousShears;

impl RelicModel for PrecariousShears {
    fn name(&self) -> &'static str {
        "Precarious Shears"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().removable_cards();
        ctx.select(SelectPurpose::Remove { damage: DAMAGE }, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for PrecariousShears {
    type Me = RelicId;
}
