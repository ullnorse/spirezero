//! `Crossbow` (`Models.Relics/Crossbow.cs`): Ancient (Tanx); at the start of each of the owner's
//! turns a random Attack of the character pool, free this turn, joins the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, RelicId, Side};
use crate::content::cards::{CardType, pools};
use crate::content::relics::RelicModel;

pub struct Crossbow;

impl RelicModel for Crossbow {
    fn may_prompt(&self) -> bool {
        // The generated card can be one that prompts when played.
        true
    }

    fn name(&self) -> &'static str {
        "Crossbow"
    }
}

#[sts2_macros::hooks]
impl Hooks for Crossbow {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if !participants.contains(&owner) {
            return Ok(());
        }
        let attacks: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Attack)
            .collect();
        if attacks.is_empty() {
            return Ok(());
        }
        let card = eng.get_distinct_for_combat(&attacks, 1)?[0];
        // SetToFreeThisTurn.
        eng.s.set_to_free_this_turn(card);
        eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))
    }
}
