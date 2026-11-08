//! `Glitterstream` (`Models.Cards/Glitterstream.cs`): Regent common skill, cost 2; gain 11 block (13 upgraded), then 5 block (7 upgraded) next turn. The next-turn amount goes through `Hook.ModifyBlock` first.

use crate::combat::hooks::{BlockQuery, Hooks};
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Glitterstream;

#[sts2_macros::hooks]
impl Hooks for Glitterstream {
    type Me = CardId;
}

impl CardModel for Glitterstream {
    fn name(&self) -> &'static str {
        "Glitterstream"
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
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 11).upgrade(2),
            CardVar::new(Var::Named("BlockNextTurn"), 5)
                .block()
                .upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let next_turn = eng.s.card(play.card).var(Var::Named("BlockNextTurn"));
        let query = BlockQuery {
            target: player,
            props: ValueProps::MOVE,
            card: Some(play.card),
            play: Some(*play),
        };
        let (block_next_turn, _) = eng.hooked().modify_block(query, next_turn);
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::BlockNextTurn,
            block_next_turn,
            Some(player),
        )?;
        Ok(())
    }
}
