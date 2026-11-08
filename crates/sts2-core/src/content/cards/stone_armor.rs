//! `StoneArmor` (`Models.Cards/StoneArmor.cs`): cost 1, applies `PlatingPower` 4 (6 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct StoneArmor;

#[sts2_macros::hooks]
impl Hooks for StoneArmor {
    type Me = CardId;
}

impl CardModel for StoneArmor {
    fn name(&self) -> &'static str {
        "Stone Armor"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Plating), 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Plating,
            eng.s.card(play.card).var(Var::Power(PowerKind::Plating)),
            Some(player),
        )
    }
}
