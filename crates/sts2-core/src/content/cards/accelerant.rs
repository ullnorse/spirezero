//! `Accelerant` (`Models.Cards/Accelerant.cs`): cost 1, applies 1 Accelerant (2 upgraded) to the player.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Accelerant;

#[sts2_macros::hooks]
impl Hooks for Accelerant {
    type Me = CardId;
}

impl CardModel for Accelerant {
    fn name(&self) -> &'static str {
        "Accelerant"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Accelerant"), 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        // DynamicVar("Accelerant")(1m), +1.
        let amount = eng.s.card(play.card).var(Var::Named("Accelerant"));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Accelerant,
            amount,
            Some(player),
        )
    }
}
