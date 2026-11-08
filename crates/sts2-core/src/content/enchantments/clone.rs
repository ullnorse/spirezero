//! `Clone` (`Models.Enchantments/Clone.cs`): no effect of its own; the `CLONE` rest-site option
//! (`PaelsGrowth`) copies every deck card that has it (`CloneRestSiteOption.cs`).

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentModel;

/// Marks a deck card for the `CLONE` rest-site option.
pub struct Clone;

impl EnchantmentModel for Clone {
    fn name(&self) -> &'static str {
        "Clone"
    }
}

#[sts2_macros::hooks]
impl Hooks for Clone {
    type Me = CardId;
}
