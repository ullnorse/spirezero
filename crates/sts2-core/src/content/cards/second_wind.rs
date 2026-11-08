//! `SecondWind` (`Models.Cards/SecondWind.cs`): exhaust every non-Attack card of the hand, gaining
//! 5 block (7 upgraded) for each.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct SecondWind;

#[sts2_macros::hooks]
impl Hooks for SecondWind {
    type Me = CardId;
}

impl CardModel for SecondWind {
    fn name(&self) -> &'static str {
        "Second Wind"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 5).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let cards: Vec<CardId> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).card_type() != CardType::Attack)
            .collect();
        let player = eng.s.player.creature;
        for card in cards {
            eng.exhaust(card, false)?;
            eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        }
        Ok(())
    }
}
