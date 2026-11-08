//! `WraithForm` (`Models.Cards/WraithForm.cs`): cost 3; 2 Intangible (3 upgraded) and 1 Wraith Form (lose 1 Dexterity each turn).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct WraithForm;

#[sts2_macros::hooks]
impl Hooks for WraithForm {
    type Me = CardId;
}

impl CardModel for WraithForm {
    fn name(&self) -> &'static str {
        "Wraith Form"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Power
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Intangible), 2).upgrade(1),
            CardVar::new(Var::Power(PowerKind::WraithForm), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Intangible,
            eng.s.card(play.card).var(Var::Power(PowerKind::Intangible)),
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::WraithForm,
            eng.s.card(play.card).var(Var::Power(PowerKind::WraithForm)),
            Some(player),
        )
    }
}
