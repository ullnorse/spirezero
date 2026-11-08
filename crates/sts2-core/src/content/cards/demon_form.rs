//! `DemonForm` (`Models.Cards/DemonForm.cs`): cost 3, gain 3 Strength (4 upgraded) at the start of each turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct DemonForm;

#[sts2_macros::hooks]
impl Hooks for DemonForm {
    type Me = CardId;
}

impl CardModel for DemonForm {
    fn name(&self) -> &'static str {
        "Demon Form"
    }

    fn cost(&self) -> i32 {
        3
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Strength), 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::DemonForm,
            eng.s.card(play.card).var(Var::Power(PowerKind::Strength)),
            Some(player),
        )
    }
}
