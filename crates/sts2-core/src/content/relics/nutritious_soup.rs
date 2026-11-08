//! `NutritiousSoup` (`Models.Relics/NutritiousSoup.cs`): Ancient (Tezcatara); `AfterObtained`
//! enchants every Basic, Strike-tagged deck card `TezcatarasEmber` can enchant with it (amount 1),
//! in deck order.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct NutritiousSoup;

/// `Rarity == Basic && Tags.Contains(Strike)`.
#[must_use]
pub fn is_basic_strike(card: RunCard) -> bool {
    card.is_basic() && card.to_combat().is_some_and(|c| c.kind.is_strike())
}

impl RelicModel for NutritiousSoup {
    fn name(&self) -> &'static str {
        "Nutritious Soup"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        for i in 0..s.deck.len() {
            if is_basic_strike(s.deck[i])
                && s.deck[i].can_enchant(EnchantmentKind::TezcatarasEmber)?
            {
                s.enchant(i, EnchantmentKind::TezcatarasEmber, 1)?;
            }
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for NutritiousSoup {
    type Me = RelicId;
}
