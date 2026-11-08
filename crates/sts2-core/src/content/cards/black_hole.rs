//! `BlackHole` (`Models.Cards/BlackHole.cs`): Regent uncommon power, cost 1; gain 3 Black Hole (4 upgraded): damage to all enemies after spending or gaining stars.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct BlackHole;

#[sts2_macros::hooks]
impl Hooks for BlackHole {
    type Me = CardId;
}

impl CardModel for BlackHole {
    fn name(&self) -> &'static str {
        "Black Hole"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::BlackHole), 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::BlackHole));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::BlackHole,
            amount,
            Some(player),
        )
    }
}
