//! `Anticipate` (`Models.Cards/Anticipate.cs`): cost 0, 2 Dexterity this turn (4 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Anticipate;

#[sts2_macros::hooks]
impl Hooks for Anticipate {
    type Me = CardId;
}

impl CardModel for Anticipate {
    fn name(&self) -> &'static str {
        "Anticipate"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Dexterity), 2).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Anticipate,
            eng.s.card(play.card).var(Var::Power(PowerKind::Dexterity)),
            Some(player),
        )
    }
}
