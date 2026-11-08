//! `Equilibrium` (`Models.Cards/Equilibrium.cs`): cost 2; 13 Block (16 upgraded), then 1 Retain
//! Hand.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Equilibrium;

#[sts2_macros::hooks]
impl Hooks for Equilibrium {
    type Me = CardId;
}

impl CardModel for Equilibrium {
    fn name(&self) -> &'static str {
        "Equilibrium"
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
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 13).upgrade(3),
            CardVar::new(Var::Named("Equilibrium"), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let turns = eng.s.card(play.card).var(Var::Named("Equilibrium"));
        eng.apply_power(player, PowerKind::RetainHand, turns, Some(player))
    }
}
