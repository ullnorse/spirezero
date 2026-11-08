//! `VexingPuzzlebox` (`Models.Relics/VexingPuzzlebox.cs`): at the start of the first turn a
//! random card of the character pool, free this turn, joins the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, RelicId};
use crate::content::cards::pools;
use crate::content::relics::RelicModel;

pub struct VexingPuzzlebox;

impl RelicModel for VexingPuzzlebox {
    fn may_prompt(&self) -> bool {
        // The generated card can be one that prompts when played.
        true
    }

    fn name(&self) -> &'static str {
        "Vexing Puzzlebox"
    }
}

#[sts2_macros::hooks]
impl Hooks for VexingPuzzlebox {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number != 1 {
            return Ok(());
        }
        let owner = eng.s.player.creature;
        let card = eng.get_distinct_for_combat(&pools::character(eng.s.player.character), 1)?[0];
        // SetToFreeThisTurn.
        eng.s.set_to_free_this_turn(card);
        eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))
    }
}
