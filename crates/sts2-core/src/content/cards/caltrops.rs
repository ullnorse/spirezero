//! `Caltrops` (`Models.Cards/Caltrops.cs`): Event power, cost 1; gain 3 Thorns (5 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Caltrops;

#[sts2_macros::hooks]
impl Hooks for Caltrops {
    type Me = CardId;
}

impl CardModel for Caltrops {
    fn name(&self) -> &'static str {
        "Caltrops"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Thorns), 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let thorns = eng.s.card(play.card).var(Var::Power(PowerKind::Thorns));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Thorns,
            thorns,
            Some(player),
        )
    }
}
