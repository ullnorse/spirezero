//! `RollingBoulder` (`Models.Cards/RollingBoulder.cs`): power, cost 3; apply a new
//! `RollingBoulderPower` instance of 5 (10 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct RollingBoulder;

#[sts2_macros::hooks]
impl Hooks for RollingBoulder {
    type Me = CardId;
}

impl CardModel for RollingBoulder {
    fn name(&self) -> &'static str {
        "Rolling Boulder"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::RollingBoulder), 5).upgrade(5),
            CardVar::new(Var::Named("IncrementAmount"), 5),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng
            .s
            .card(play.card)
            .var(Var::Power(PowerKind::RollingBoulder));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::RollingBoulder, amount, Some(player))
    }
}
