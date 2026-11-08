//! `BurningPact` (`Models.Cards/BurningPact.cs`): exhaust a card of the hand (the player's pick),
//! then draw 2 (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct BurningPact;

#[sts2_macros::hooks]
impl Hooks for BurningPact {
    type Me = CardId;
}

impl CardModel for BurningPact {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Burning Pact"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // CardSelectCmd.FromHand(CardSelectorPrefs(ExhaustSelectionPrompt, 1)).FirstOrDefault().
        let options = eng.s.player.hand.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Exhaust), options, 1, 1)?
            .first()
        {
            eng.exhaust(card, false)?;
        }
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        eng.draw(Decimal::from(cards), false)?;
        Ok(())
    }
}
