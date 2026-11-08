//! `Impervious` (`Models.Cards/Impervious.cs`): cost 2, Exhaust, 30 block (40 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Impervious;

#[sts2_macros::hooks]
impl Hooks for Impervious {
    type Me = CardId;
}

impl CardModel for Impervious {
    fn name(&self) -> &'static str {
        "Impervious"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn gains_block(&self) -> bool {
        true
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 30).upgrade(10)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}
