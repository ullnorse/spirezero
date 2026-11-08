//! `PrepTime` (`Models.Cards/PrepTime.cs`): power, cost 1; apply `PrepTimePower` 4 (6 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct PrepTime;

#[sts2_macros::hooks]
impl Hooks for PrepTime {
    type Me = CardId;
}

impl CardModel for PrepTime {
    fn name(&self) -> &'static str {
        "Prep Time"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::PrepTime), 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::PrepTime));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::PrepTime, amount, Some(player))
    }
}
