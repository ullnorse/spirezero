//! `Calcify` (`Models.Cards/Calcify.cs`): cost 1 power; applies 4 Calcify (6 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Calcify;

#[sts2_macros::hooks]
impl Hooks for Calcify {
    type Me = CardId;
}

impl CardModel for Calcify {
    fn name(&self) -> &'static str {
        "Calcify"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Calcify), 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Calcify));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::Calcify, amount, Some(player))
    }
}
