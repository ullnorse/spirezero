//! `DanseMacabre` (`Models.Cards/DanseMacabre.cs`): cost 1; whenever a card costing 2 or more is played, gain 4 Block (6 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct DanseMacabre;

#[sts2_macros::hooks]
impl Hooks for DanseMacabre {
    type Me = CardId;
}

impl CardModel for DanseMacabre {
    fn name(&self) -> &'static str {
        "Danse Macabre"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::DanseMacabre), 4).upgrade(2),
            CardVar::new(Var::Energy, 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng
            .s
            .card(play.card)
            .var(Var::Power(PowerKind::DanseMacabre));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::DanseMacabre, amount, Some(player))
    }
}
