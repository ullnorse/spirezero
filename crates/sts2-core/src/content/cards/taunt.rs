//! `Taunt` (`Models.Cards/Taunt.cs`): cost 1, 6 Block (7 upgraded) and 1 Vulnerable (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Taunt;

#[sts2_macros::hooks]
impl Hooks for Taunt {
    type Me = CardId;
}

impl CardModel for Taunt {
    fn name(&self) -> &'static str {
        "Taunt"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 6).upgrade(1),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Taunt needs a target");
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable)),
            Some(player),
        )
    }
}
