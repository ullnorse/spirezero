//! `FlameBarrier` (`Models.Cards/FlameBarrier.cs`): cost 2, 12 Block (16 upgraded) and `FlameBarrierPower` 4 (6 upgraded): attackers take damage back until your next turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct FlameBarrier;

#[sts2_macros::hooks]
impl Hooks for FlameBarrier {
    type Me = CardId;
}

impl CardModel for FlameBarrier {
    fn name(&self) -> &'static str {
        "Flame Barrier"
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
            CardVar::new(Var::Block, 12).upgrade(4),
            CardVar::new(Var::Named("DamageBack"), 4).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.gain_block(
            player,
            eng.s.card(play.card).var(Var::Block),
            ValueProps::MOVE,
            Some(play),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::FlameBarrier,
            eng.s.card(play.card).var(Var::Named("DamageBack")),
            Some(player),
        )
    }
}
