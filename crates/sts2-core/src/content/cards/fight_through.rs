//! `FightThrough` (`Models.Cards/FightThrough.cs`): cost 1, 13 block (17 upgraded), then two `Wound`s into the discard pile.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, ValueProps};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct FightThrough;

#[sts2_macros::hooks]
impl Hooks for FightThrough {
    type Me = CardId;
}

impl CardModel for FightThrough {
    fn name(&self) -> &'static str {
        "Fight Through"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 13).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        for _ in 0..2 {
            let card = eng.create_card(CardKind::Wound);
            eng.add_generated_card_to_combat(card, PileType::Discard, Some(player))?;
        }
        Ok(())
    }
}
