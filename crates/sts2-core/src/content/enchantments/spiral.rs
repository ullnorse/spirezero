//! `Spiral` (`Models.Enchantments/Spiral.cs`): only a Basic Strike or Defend can have it; its card
//! is played `Times` (`IntVar("Times", 1m)`, independent of `Amount`) more times.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::content::cards::CardRarity;
use crate::content::enchantments::{Enchantment, EnchantmentModel};

/// `DynamicVars["Times"]`.
const TIMES: u32 = 1;

/// Replays its card once more.
pub struct Spiral;

impl EnchantmentModel for Spiral {
    fn name(&self) -> &'static str {
        "Spiral"
    }

    fn can_enchant_extra(&self, card: &Card) -> bool {
        card.kind.rarity() == CardRarity::Basic && (card.kind.is_strike() || card.kind.is_defend())
    }

    fn enchant_play_count(&self, _enchantment: Enchantment, original: u32) -> u32 {
        original + TIMES
    }
}

#[sts2_macros::hooks]
impl Hooks for Spiral {
    type Me = CardId;
}
