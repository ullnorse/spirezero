//! `HandTrick` (`Models.Cards/HandTrick.cs`): cost 1, Skill, Uncommon; 7 Block (10 upgraded),
//! then picks one Skill in the hand that is not Sly this turn and makes it Sly for this turn
//! (`CardCmd.ApplySingleTurnSly`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct HandTrick;

#[sts2_macros::hooks]
impl Hooks for HandTrick {
    type Me = CardId;
}

impl CardModel for HandTrick {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Hand Trick"
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
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 7).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        // FromHand(prefs: 1, filter: Skill && !IsSlyThisTurn): exactly one card, if any.
        let hooked = eng.hooked();
        let options: Vec<CardId> = hooked
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| {
                hooked.s.card(c).card_type() == CardType::Skill && !hooked.is_sly_this_turn(c)
            })
            .collect();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.apply_single_turn_sly(card);
        }
        Ok(())
    }
}
