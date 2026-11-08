//! `Delay` (`Models.Cards/Delay.cs`): cost 2, 11 Block (13 upgraded) and 1 energy (2) next turn.

use crate::combat::ValueProps;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Delay;

#[sts2_macros::hooks]
impl Hooks for Delay {
    type Me = CardId;
}

impl CardModel for Delay {
    fn name(&self) -> &'static str {
        "Delay"
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
            CardVar::new(Var::Block, 11).upgrade(2),
            CardVar::new(Var::Energy, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let energy = eng.s.card(play.card).var(Var::Energy);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power(player, PowerKind::EnergyNextTurn, energy, Some(player))
    }
}
