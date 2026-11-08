//! `GamblingChip` (`Models.Relics/GamblingChip.cs`): after the first turn's hand draw, discard
//! any number of cards from the hand, then draw that many.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, Prompt, PromptText, RelicId};
use crate::content::relics::RelicModel;

pub struct GamblingChip;

impl RelicModel for GamblingChip {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Gambling Chip"
    }
}

#[sts2_macros::hooks]
impl Hooks for GamblingChip {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        // `player == Owner`: the only player.
        if eng.s.player.turn_number > 1 {
            return Ok(());
        }
        // FromHandForDiscard(.., CardSelectorPrefs(prompt, 0, 999999999), null): at most the
        // hand.
        let options = eng.s.player.hand.to_vec();
        let max = options.len();
        let cards = eng.choose_cards(Prompt::relic(me, PromptText::Own), options, 0, max)?;
        if cards.is_empty() {
            return Ok(());
        }
        let draw = i32::try_from(cards.len()).expect("hand size fits i32");
        eng.discard_and_draw(&cards, draw)
    }
}
