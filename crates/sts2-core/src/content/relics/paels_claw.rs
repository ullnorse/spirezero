//! `PaelsClaw` (`Models.Relics/PaelsClaw.cs`): Ancient (Pael); `AfterObtained` enchants every deck
//! card `Goopy` can enchant (the Defend-tagged ones) with `Goopy` 1, in deck order.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCtx, RunError};

pub struct PaelsClaw;

impl RelicModel for PaelsClaw {
    fn name(&self) -> &'static str {
        "Pael's Claw"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        for i in s.enchantable_cards(EnchantmentKind::Goopy)? {
            s.enchant(i, EnchantmentKind::Goopy, 1)?;
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsClaw {
    type Me = RelicId;
}
