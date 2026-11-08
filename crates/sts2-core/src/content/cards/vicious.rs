//! `Vicious` (`Models.Cards/Vicious.cs`): cost 1, draw 1 card (2 upgraded) whenever you apply Vulnerable.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Vicious;

#[sts2_macros::hooks]
impl Hooks for Vicious {
    type Me = CardId;
}

impl CardModel for Vicious {
    fn name(&self) -> &'static str {
        "Vicious"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Vicious,
            eng.s.card(play.card).var(Var::Cards),
            Some(player),
        )
    }
}
