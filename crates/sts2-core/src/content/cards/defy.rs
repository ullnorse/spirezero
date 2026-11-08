//! `Defy` (`Models.Cards/Defy.cs`): cost 1, Ethereal, 6 Block (9 upgraded) and 1 Weak.

use crate::combat::ValueProps;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Defy;

#[sts2_macros::hooks]
impl Hooks for Defy {
    type Me = CardId;
}

impl CardModel for Defy {
    fn name(&self) -> &'static str {
        "Defy"
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
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 6).upgrade(3),
            CardVar::new(Var::Power(PowerKind::Weak), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Defy needs a target");
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Weak,
            eng.s.card(play.card).var(Var::Power(PowerKind::Weak)),
            Some(player),
        )
    }
}
