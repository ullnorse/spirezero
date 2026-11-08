//! `Slither` (`Models.Enchantments/Slither.cs`): when its card is drawn into the hand, the card
//! costs `Rng.CombatEnergyCosts.NextInt(4)` for the rest of combat. Cannot enchant Unplayable or
//! X-cost cards.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, Engine, Flow, PileType};
use crate::content::cards::CardKeyword;
use crate::content::enchantments::EnchantmentModel;
use crate::rng::RunRngType;

/// Draws at a random cost 0-3.
pub struct Slither;

impl EnchantmentModel for Slither {
    fn name(&self) -> &'static str {
        "Slither"
    }

    /// The local keywords: global ones (`Hook.ModifyKeywordsInCombat`) never add Unplayable.
    fn can_enchant_extra(&self, card: &Card) -> bool {
        !card.has_local(CardKeyword::Unplayable) && !card.energy_cost.costs_x
    }
}

#[sts2_macros::hooks]
impl Hooks for Slither {
    type Me = CardId;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        if card != me || eng.s.pile_of(me) != Some(PileType::Hand) {
            return Ok(());
        }
        let cost = eng.s.rng[RunRngType::CombatEnergyCosts].next_int(4);
        eng.s.energy_cost_mut(me).set_this_combat(cost);
        Ok(())
    }
}
