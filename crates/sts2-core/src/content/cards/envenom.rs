//! `Envenom` (`Models.Cards/Envenom.cs`): cost 2, applies 1 Envenom (2 upgraded) to the player.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Envenom;

#[sts2_macros::hooks]
impl Hooks for Envenom {
    type Me = CardId;
}

impl CardModel for Envenom {
    fn name(&self) -> &'static str {
        "Envenom"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Envenom), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Envenom));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Envenom,
            amount,
            Some(player),
        )
    }
}
