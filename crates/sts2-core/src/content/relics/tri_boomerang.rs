//! `TriBoomerang` (`Models.Relics/TriBoomerang.cs`): Ancient (Tanx); `AfterObtained` enchants 3
//! chosen deck cards with `Instinct` (`FromDeckForEnchantment`, exactly 3).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct TriBoomerang;

impl RelicModel for TriBoomerang {
    fn name(&self) -> &'static str {
        "Tri-Boomerang"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let options = ctx.state().enchantable_cards(EnchantmentKind::Instinct)?;
        ctx.select(
            SelectPurpose::Enchant {
                kind: EnchantmentKind::Instinct,
                amount: 1,
            },
            options,
            CARDS,
        )
    }
}

#[sts2_macros::hooks]
impl Hooks for TriBoomerang {
    type Me = RelicId;
}
