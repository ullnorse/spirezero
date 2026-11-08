//! `Finesse` (`Models.Cards/Finesse.cs`): cost 0; 4 block (7 upgraded), then draw 1.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Finesse;

#[sts2_macros::hooks]
impl Hooks for Finesse {
    type Me = CardId;
}

impl CardModel for Finesse {
    fn name(&self) -> &'static str {
        "Finesse"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 4).upgrade(3),
            CardVar::new(Var::Cards, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
