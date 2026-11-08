//! `Kifuda` (`Models.Relics/Kifuda.cs`): `AfterObtained` lets the player pick up to 3 deck cards
//! `Adroit` can enchant (`FromDeckForEnchantment`, `MinSelect` 0, manual confirmation) and
//! enchants them with `Adroit` 3.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(3)`.
const CARDS: usize = 3;
/// `_adroitAmount`.
const ADROIT_AMOUNT: i32 = 3;

pub struct Kifuda;

impl RelicModel for Kifuda {
    fn name(&self) -> &'static str {
        "Kifuda"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().enchantable_cards(EnchantmentKind::Adroit)?;
        ctx.select_up_to(
            SelectPurpose::EnchantUpTo {
                kind: EnchantmentKind::Adroit,
                amount: ADROIT_AMOUNT,
            },
            options,
            CARDS,
        );
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Kifuda {
    type Me = RelicId;
}
