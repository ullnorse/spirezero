//! `Pyre` (`Models.Cards/Pyre.cs`): cost 2; apply `PyrePower` (1 energy, 2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Pyre;

#[sts2_macros::hooks]
impl Hooks for Pyre {
    type Me = CardId;
}

impl CardModel for Pyre {
    fn name(&self) -> &'static str {
        "Pyre"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Energy, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let energy = eng.s.card(play.card).int_var(Var::Energy);
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Pyre,
            Decimal::from(energy),
            Some(player),
        )
    }
}
