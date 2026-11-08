//! `OrangeDough` (`Models.Relics/OrangeDough.cs`): at the start of the first turn, 2 distinct
//! random Colorless cards are created in the hand.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, RelicId, Side};
use crate::content::cards::pools;
use crate::content::relics::RelicModel;

/// `CardsVar(2)`.
const CARDS: usize = 2;

pub struct OrangeDough;

impl RelicModel for OrangeDough {
    fn may_prompt(&self) -> bool {
        // Generated Colorless cards prompt (Discovery, ...).
        true
    }

    fn name(&self) -> &'static str {
        "Orange Dough"
    }
}

#[sts2_macros::hooks]
impl Hooks for OrangeDough {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.player.turn_number <= 1 {
            // Flash: visual only.
            let cards = eng.get_distinct_for_combat(&pools::colorless(), CARDS)?;
            eng.add_generated_cards_to_combat(
                &cards,
                PileType::Hand,
                Some(owner),
                Position::Bottom,
            )?;
        }
        Ok(())
    }
}
