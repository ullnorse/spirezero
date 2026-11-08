//! `PaleBlueDot` (`Models.Cards/PaleBlueDot.cs`): Regent uncommon power, cost 1; apply
//! `PaleBlueDotPower` with 1 (2 upgraded) card. `CardPlay` (5) is the power's threshold.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct PaleBlueDot;

#[sts2_macros::hooks]
impl Hooks for PaleBlueDot {
    type Me = CardId;
}

impl CardModel for PaleBlueDot {
    fn name(&self) -> &'static str {
        "Pale Blue Dot"
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
            CardVar::new(Var::Cards, 1).upgrade(1),
            CardVar::new(Var::Named("CardPlay"), 5),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).var(Var::Cards);
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::PaleBlueDot,
            cards,
            Some(player),
        )
    }
}
