//! `TrueGrit` (`Models.Cards/TrueGrit.cs`): 7 block (9 upgraded), then exhaust a card of the hand:
//! the player's pick when upgraded, else a random one (`Rng.CombatCardSelection`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::rng::RunRngType;

pub struct TrueGrit;

#[sts2_macros::hooks]
impl Hooks for TrueGrit {
    type Me = CardId;
}

impl CardModel for TrueGrit {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "True Grit"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 7).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let upgraded = eng.s.card(play.card).is_upgraded();
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let hand = eng.s.player.hand.to_vec();
        let picked = if upgraded {
            // CardSelectCmd.FromHand(CardSelectorPrefs(ExhaustSelectionPrompt, 1)).FirstOrDefault().
            eng.choose_cards(Prompt::card(play.card, PromptText::Exhaust), hand, 1, 1)?
                .first()
                .copied()
        } else {
            eng.s.rng[RunRngType::CombatCardSelection]
                .next_item(&hand)
                .copied()
        };
        if let Some(card) = picked {
            eng.exhaust(card, false)?;
        }
        Ok(())
    }
}
