//! `Goopy` (`Models.Enchantments/Goopy.cs`): Defend-tagged cards only; `OnEnchant` adds Exhaust.
//! Every play of the card raises `Amount` by 1, on the combat card and on its deck version
//! (`Card.DeckVersion.Enchantment.Amount++`), and the card gains `Amount - 1` extra block.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::CardKeyword;
use crate::content::enchantments::{Enchantment, EnchantmentModel};
use crate::decimal::Decimal;

/// Exhausts; each play adds 1 block to the card's later plays, for the rest of the run.
pub struct Goopy;

impl EnchantmentModel for Goopy {
    fn name(&self) -> &'static str {
        "Goopy"
    }

    /// `Card.Tags.Contains(CardTag.Defend)` (`Goopy.cs:22-29`).
    fn can_enchant_extra(&self, card: &Card) -> bool {
        card.kind.is_defend()
    }

    fn on_enchant(&self, card: &mut Card) {
        card.keywords.insert(CardKeyword::Exhaust);
    }

    /// `Amount - 1` (`Goopy.cs:51-54`).
    fn enchant_block_additive(&self, enchantment: Enchantment, _block: Decimal) -> Decimal {
        Decimal::from(enchantment.amount - 1)
    }
}

#[sts2_macros::hooks]
impl Hooks for Goopy {
    type Me = CardId;

    /// `Goopy.cs:36-49`.
    fn after_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        if play.card != me {
            return Ok(());
        }
        let card = eng.s.card_mut(me);
        if let Some(enchantment) = card.enchantment.as_mut() {
            enchantment.amount += 1;
        }
        if let Some(i) = card.deck_version()
            && let Some(enchantment) = eng.s.deck[i].enchantment.as_mut()
        {
            enchantment.amount += 1;
        }
        Ok(())
    }
}
