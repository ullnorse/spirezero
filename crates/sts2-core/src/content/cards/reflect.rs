//! `Reflect` (`Models.Cards/Reflect.cs`): Regent uncommon skill, cost 1 and 3 stars; gain 15 block (20 upgraded) and 1 Reflect.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Reflect;

#[sts2_macros::hooks]
impl Hooks for Reflect {
    type Me = CardId;
}

impl CardModel for Reflect {
    fn name(&self) -> &'static str {
        "Reflect"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn star_cost(&self) -> i32 {
        3
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 15).upgrade(5)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Reflect,
            Decimal::ONE,
            Some(player),
        )
    }
}
