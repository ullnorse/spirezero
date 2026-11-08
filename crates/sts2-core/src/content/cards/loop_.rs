//! `Loop` (`Models.Cards/Loop.cs`): power, cost 1; apply `LoopPower` (`Loop` var 1, +1 upgraded).
//! The file is `loop_.rs` because `loop` is a Rust keyword.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Loop;

#[sts2_macros::hooks]
impl Hooks for Loop {
    type Me = CardId;
}

impl CardModel for Loop {
    fn name(&self) -> &'static str {
        "Loop"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Loop"), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).int_var(Var::Named("Loop"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Loop,
            Decimal::from(amount),
            Some(player),
        )
    }
}
