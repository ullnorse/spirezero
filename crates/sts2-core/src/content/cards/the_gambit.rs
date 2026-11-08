//! `TheGambit` (`Models.Cards/TheGambit.cs`): cost 0; 50 Block (75 upgraded), then apply
//! `TheGambitPower` (the owner dies to the next unblocked attack damage).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct TheGambit;

#[sts2_macros::hooks]
impl Hooks for TheGambit {
    type Me = CardId;
}

impl CardModel for TheGambit {
    fn name(&self) -> &'static str {
        "The Gambit"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 50).upgrade(25)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power(player, PowerKind::TheGambit, Decimal::ONE, Some(player))
    }
}
