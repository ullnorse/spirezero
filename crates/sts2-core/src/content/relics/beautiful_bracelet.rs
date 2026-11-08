//! `BeautifulBracelet` (`Models.Relics/BeautifulBracelet.cs`): Ancient (Nonupeipe);
//! `AfterObtained` enchants 4 random deck cards `Swift` can enchant with `Swift` 2
//! (`TakeRandom` on `Rng.Niche`: the whole list is shuffled, then the first 4 are taken).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentKind;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{RunCtx, RunError};
use crate::shuffle::unstable_shuffle;

/// `CardsVar(4)`.
const CARDS: usize = 4;
/// `DynamicVar("Swift", 2m)`.
const SWIFT: i32 = 2;

pub struct BeautifulBracelet;

impl RelicModel for BeautifulBracelet {
    fn name(&self) -> &'static str {
        "Beautiful Bracelet"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let mut cards = s.enchantable_cards(EnchantmentKind::Swift)?;
        unstable_shuffle(&mut cards, &mut s.rng[RunRngType::Niche]);
        for &i in cards.iter().take(CARDS) {
            s.enchant(i, EnchantmentKind::Swift, SWIFT)?;
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for BeautifulBracelet {
    type Me = RelicId;
}
