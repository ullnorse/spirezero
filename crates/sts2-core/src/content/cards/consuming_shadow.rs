//! `ConsumingShadow` (`Models.Cards/ConsumingShadow.cs`): power, cost 2; channel `Repeat` (2, 3
//! upgraded) Dark, then apply `ConsumingShadowPower` (1).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct ConsumingShadow;

#[sts2_macros::hooks]
impl Hooks for ConsumingShadow {
    type Me = CardId;
}

impl CardModel for ConsumingShadow {
    fn name(&self) -> &'static str {
        "Consuming Shadow"
    }

    fn cost(&self) -> i32 {
        2
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
            CardVar::new(Var::Repeat, 2).upgrade(1),
            CardVar::new(Var::Power(PowerKind::ConsumingShadow), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let repeat = eng.s.card(play.card).int_var(Var::Repeat);
        let amount = eng
            .s
            .card(play.card)
            .int_var(Var::Power(PowerKind::ConsumingShadow));
        for _ in 0..repeat {
            eng.channel(OrbKind::Dark)?;
        }
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::ConsumingShadow,
            Decimal::from(amount),
            Some(player),
        )
    }
}
