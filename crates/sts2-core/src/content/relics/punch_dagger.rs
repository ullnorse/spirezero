//! `PunchDagger` (`Models.Relics/PunchDagger.cs`): `AfterObtained` enchants 1 chosen deck card
//! `Momentum` can enchant with `Momentum` 5 (`FromDeckForEnchantment`, one pick).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `DynamicVar("Momentum", 5m)`.
const MOMENTUM_AMOUNT: i32 = 5;

pub struct PunchDagger;

impl RelicModel for PunchDagger {
    fn name(&self) -> &'static str {
        "Punch Dagger"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().enchantable_cards(EnchantmentKind::Momentum)?;
        ctx.select(
            SelectPurpose::Enchant {
                kind: EnchantmentKind::Momentum,
                amount: MOMENTUM_AMOUNT,
            },
            options,
            1,
        )
    }
}

#[sts2_macros::hooks]
impl Hooks for PunchDagger {
    type Me = RelicId;
}
