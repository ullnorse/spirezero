//! `SleightOfFlesh` (`Models.Cards/SleightOfFlesh.cs`): cost 2 power; applies
//! `SleightOfFleshPower` 9 (13 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct SleightOfFlesh;

#[sts2_macros::hooks]
impl Hooks for SleightOfFlesh {
    type Me = CardId;
}

impl CardModel for SleightOfFlesh {
    fn name(&self) -> &'static str {
        "Sleight of Flesh"
    }

    fn cost(&self) -> i32 {
        2
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
        const VARS: &[CardVar] =
            &[CardVar::new(Var::Power(PowerKind::SleightOfFlesh), 9).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng
            .s
            .card(play.card)
            .var(Var::Power(PowerKind::SleightOfFlesh));
        let player = eng.s.player.creature;
        eng.apply_power(player, PowerKind::SleightOfFlesh, amount, Some(player))
    }
}
