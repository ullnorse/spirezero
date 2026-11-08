//! `Guards` (`Models.Cards/Guards.cs`): Regent rare skill, cost 2, Exhaust; transform any number of
//! cards in the hand into `MinionSacrifice`s, upgraded if this card is.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardKind, CardModel, CardRarity, CardType, Keywords, TargetType, pools,
};

pub struct Guards;

#[sts2_macros::hooks]
impl Hooks for Guards {
    type Me = CardId;
}

impl CardModel for Guards {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "GUARDS!!!"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, 0, 999999999)).
        let options = eng.s.player.hand.to_vec();
        let upgraded = eng.s.card(play.card).is_upgraded();
        let max = options.len();
        for card in eng.choose_cards(Prompt::card(play.card, PromptText::Own), options, 0, max)? {
            pools::transform_to_upgraded(eng, card, CardKind::MinionSacrifice, upgraded)?;
        }
        Ok(())
    }
}
