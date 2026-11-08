//! `PanicButton` (`Models.Cards/PanicButton.cs`): cost 0, Exhaust; 30 Block (40 upgraded), then
//! `NoBlockPower` for 2 turns.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct PanicButton;

#[sts2_macros::hooks]
impl Hooks for PanicButton {
    type Me = CardId;
}

impl CardModel for PanicButton {
    fn name(&self) -> &'static str {
        "Panic Button"
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
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 30).upgrade(10),
            CardVar::new(Var::Named("Turns"), 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let turns = eng.s.card(play.card).var(Var::Named("Turns"));
        eng.apply_power(player, PowerKind::NoBlock, turns, Some(player))
    }
}
