//! `Begone` (`Models.Cards/Begone.cs`): Regent common skill, cost 1; transform 1 card in the hand
//! into a `MinionStrike`, upgraded if this card is.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, TargetType, pools};

pub struct Begone;

#[sts2_macros::hooks]
impl Hooks for Begone {
    type Me = CardId;
}

impl CardModel for Begone {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "BEGONE!"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromHand(CardSelectorPrefs(TransformSelectionPrompt, 1)).FirstOrDefault().
        let options = eng.s.player.hand.to_vec();
        if let Some(&card) = eng
            .choose_cards(
                Prompt::card(play.card, PromptText::Transform),
                options,
                1,
                1,
            )?
            .first()
        {
            let upgraded = eng.s.card(play.card).is_upgraded();
            pools::transform_to_upgraded(eng, card, CardKind::MinionStrike, upgraded)?;
        }
        Ok(())
    }
}
