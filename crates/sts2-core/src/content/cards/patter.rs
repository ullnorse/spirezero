//! `Patter` (`Models.Cards/Patter.cs`): Regent common skill, cost 1; gain 8 block (10 upgraded) and 2 Vigor (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Patter;

#[sts2_macros::hooks]
impl Hooks for Patter {
    type Me = CardId;
}

impl CardModel for Patter {
    fn name(&self) -> &'static str {
        "Patter"
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
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 8).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Vigor), 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Vigor,
            eng.s.card(play.card).var(Var::Power(PowerKind::Vigor)),
            Some(player),
        )?;
        Ok(())
    }
}
