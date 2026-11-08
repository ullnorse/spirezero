//! `ChoicesParadox` (`Models.Relics/ChoicesParadox.cs`): Ancient (Vakuu); at the start of the
//! first turn, 5 distinct cards of the character's pool are generated and given Retain, and the
//! one the player picks joins the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, PileType, Prompt, PromptText, RelicId};
use crate::content::cards::{CardKeyword, pools};
use crate::content::relics::RelicModel;

/// `CardsVar(5)`.
const CARDS: usize = 5;

pub struct ChoicesParadox;

impl RelicModel for ChoicesParadox {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Choices Paradox"
    }
}

#[sts2_macros::hooks]
impl Hooks for ChoicesParadox {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        if eng.s.player.turn_number != 1 {
            return Ok(());
        }
        let options =
            eng.get_distinct_for_combat(&pools::character(eng.s.player.character), CARDS)?;
        if options.is_empty() {
            return Ok(());
        }
        for &card in &options {
            eng.apply_keyword(card, CardKeyword::Retain);
        }
        let owner = eng.s.player.creature;
        // CardSelectCmd.FromSimpleGrid(cards, prefs(prompt, 1)).
        for card in eng.choose_cards(Prompt::relic(me, PromptText::Own), options, 1, 1)? {
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(owner))?;
        }
        Ok(())
    }
}
