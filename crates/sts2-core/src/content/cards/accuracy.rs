//! `Accuracy` (`Models.Cards/Accuracy.cs`): cost 1, Power, Uncommon; applies `AccuracyPower`
//! (4 Shiv damage, 6 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Accuracy;

#[sts2_macros::hooks]
impl Hooks for Accuracy {
    type Me = CardId;
}

impl CardModel for Accuracy {
    fn name(&self) -> &'static str {
        "Accuracy"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Accuracy), 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng
            .s
            .card(play.card)
            .int_var(Var::Power(PowerKind::Accuracy));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Accuracy,
            Decimal::from(amount),
            Some(player),
        )
    }
}
