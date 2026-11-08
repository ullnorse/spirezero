//! `Toolbox` (`Models.Relics/Toolbox.cs`): before the first hand draw, choose 1 of 3 random
//! Colorless cards (or none); it joins the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, PromptSource, RelicId};
use crate::content::cards::pools;
use crate::content::relics::RelicModel;

/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct Toolbox;

impl RelicModel for Toolbox {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Toolbox"
    }
}

#[sts2_macros::hooks]
impl Hooks for Toolbox {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        if eng.s.player.turn_number != 1 {
            return Ok(());
        }
        let options = eng.get_distinct_for_combat(&pools::colorless(), CARDS)?;
        if let Some(card) = eng.choose_a_card(PromptSource::Relic(me), options)? {
            let owner = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))?;
        }
        Ok(())
    }
}
