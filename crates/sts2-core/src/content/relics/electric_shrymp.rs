//! `ElectricShrymp` (`Models.Relics/ElectricShrymp.cs`): `AfterObtained` enchants 1 chosen
//! Skill with `Imbued` (`FromDeckForEnchantment`, amount 1).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError, SelectPurpose};

/// `CardSelectorPrefs(.., 1)` and the enchant amount `1m`.
const CARDS: usize = 1;

pub struct ElectricShrymp;

impl RelicModel for ElectricShrymp {
    fn name(&self) -> &'static str {
        "Electric Shrymp"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let kind = EnchantmentKind::Imbued;
        let options = ctx.state().enchantable_cards(kind)?;
        ctx.select(SelectPurpose::Enchant { kind, amount: 1 }, options, CARDS)
    }
}

#[sts2_macros::hooks]
impl Hooks for ElectricShrymp {
    type Me = RelicId;
}
