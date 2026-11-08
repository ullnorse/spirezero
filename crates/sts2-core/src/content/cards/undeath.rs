//! `Undeath` (`Models.Cards/Undeath.cs`): cost 0, 7 block (9 upgraded), then adds a clone of
//! itself to the discard pile.

use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Undeath;

#[sts2_macros::hooks]
impl Hooks for Undeath {
    type Me = CardId;
}

impl CardModel for Undeath {
    fn name(&self) -> &'static str {
        "Undeath"
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
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 7).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let clone = eng.create_clone(play.card);
        eng.add_generated_card_to_combat(clone, PileType::Discard, Some(player))?;
        Ok(())
    }
}
