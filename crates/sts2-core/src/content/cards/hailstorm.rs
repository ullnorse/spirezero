//! `Hailstorm` (`Models.Cards/Hailstorm.cs`): power, cost 1; apply `HailstormPower` (6, +2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Hailstorm;

#[sts2_macros::hooks]
impl Hooks for Hailstorm {
    type Me = CardId;
}

impl CardModel for Hailstorm {
    fn name(&self) -> &'static str {
        "Hailstorm"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Hailstorm), 6).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng
            .s
            .card(play.card)
            .int_var(Var::Power(PowerKind::Hailstorm));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Hailstorm,
            Decimal::from(amount),
            Some(player),
        )
    }
}
