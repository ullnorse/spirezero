//! Test-only enchantment with `Imbued`'s `ShouldStartAtBottomOfDrawPile` (`Imbued.cs:11`) and
//! none of its hooks: exercises the turn-1 draw pile and `ThievingHopper`'s `Imbued` check
//! before `Imbued` is ported.

#![cfg(test)]

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::EnchantmentModel;

/// Test-only: starts at the bottom of the draw pile.
pub struct TestImbued;

impl EnchantmentModel for TestImbued {
    fn name(&self) -> &'static str {
        "Test Imbued"
    }

    fn should_start_at_bottom_of_draw_pile(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for TestImbued {
    type Me = CardId;
}
