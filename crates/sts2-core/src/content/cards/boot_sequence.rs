//! `BootSequence` (`Models.Cards/BootSequence.cs`): cost 0, Innate, Exhaust, 10 block (13 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct BootSequence;

#[sts2_macros::hooks]
impl Hooks for BootSequence {
    type Me = CardId;
}

impl CardModel for BootSequence {
    fn name(&self) -> &'static str {
        "Boot Sequence"
    }

    fn cost(&self) -> i32 {
        0
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Innate)
            .with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 10).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}
