//! `GnarledHammer` (`Models.Relics/GnarledHammer.cs`): `AfterObtained` lets the player pick up to
//! 3 deck cards `Sharp` can enchant (`FromDeckForEnchantment`, `MinSelect` 0, manual
//! confirmation) and enchants them with `Sharp` 3.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(3)`.
const CARDS: usize = 3;
/// `DynamicVar("SharpAmount", 3m)`.
const SHARP_AMOUNT: i32 = 3;

pub struct GnarledHammer;

impl RelicModel for GnarledHammer {
    fn name(&self) -> &'static str {
        "Gnarled Hammer"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().enchantable_cards(EnchantmentKind::Sharp)?;
        ctx.select_up_to(
            SelectPurpose::EnchantUpTo {
                kind: EnchantmentKind::Sharp,
                amount: SHARP_AMOUNT,
            },
            options,
            CARDS,
        );
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for GnarledHammer {
    type Me = RelicId;
}
