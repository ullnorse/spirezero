//! `Footwork` (`Models.Cards/Footwork.cs`): cost 1, 2 Dexterity (3 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Footwork;

#[sts2_macros::hooks]
impl Hooks for Footwork {
    type Me = CardId;
}

impl CardModel for Footwork {
    fn name(&self) -> &'static str {
        "Footwork"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Dexterity), 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Dexterity,
            eng.s.card(play.card).var(Var::Power(PowerKind::Dexterity)),
            Some(player),
        )
    }
}
