//! `BigHat` (`Models.Relics/BigHat.cs`): at the start of the first turn, 2 distinct random
//! Ethereal cards of the character pool join the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, RelicId, Side};
use crate::content::cards::pools;
use crate::content::relics::RelicModel;

/// `CardsVar(2)`.
const CARDS: usize = 2;

pub struct BigHat;

impl RelicModel for BigHat {
    fn may_prompt(&self) -> bool {
        // Seance, an Ethereal pool card, prompts.
        true
    }

    fn name(&self) -> &'static str {
        "Big Hat"
    }
}

#[sts2_macros::hooks]
impl Hooks for BigHat {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if !participants.contains(&owner) || eng.s.player.turn_number > 1 {
            return Ok(());
        }
        let ethereal: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.ethereal)
            .collect();
        if ethereal.is_empty() {
            return Ok(());
        }
        // CardPileCmd.AddGeneratedCardsToCombat: card by card.
        for card in eng.get_distinct_for_combat(&ethereal, CARDS)? {
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))?;
        }
        Ok(())
    }
}
