//! `SoulsPower` (`Models.Enchantments/SoulsPower.cs`): only a card with a local Exhaust keyword
//! can take it, and `OnEnchant` removes that keyword. `GraveOfTheForgotten`'s Confront applies it.

use crate::combat::CardId;
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::content::cards::CardKeyword;
use crate::content::enchantments::EnchantmentModel;

/// Takes away the Exhaust of its card.
pub struct SoulsPower;

impl EnchantmentModel for SoulsPower {
    fn name(&self) -> &'static str {
        "Soul's Power"
    }

    /// `GetKeywordsWithSources(Local).Contains(Exhaust)`. The base `CanEnchant` refuses an
    /// Unplayable card only in the deck, where the event looks for its cards.
    fn can_enchant_extra(&self, card: &Card) -> bool {
        card.has_local(CardKeyword::Exhaust) && !card.has_local(CardKeyword::Unplayable)
    }

    fn on_enchant(&self, card: &mut Card) {
        card.keywords.remove(CardKeyword::Exhaust);
    }
}

#[sts2_macros::hooks]
impl Hooks for SoulsPower {
    type Me = CardId;
}
