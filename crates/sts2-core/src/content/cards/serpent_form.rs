//! `SerpentForm` (`Models.Cards/SerpentForm.cs`): cost 3 power; after each card played, 4 damage
//! (6 upgraded) to a random enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct SerpentForm;

#[sts2_macros::hooks]
impl Hooks for SerpentForm {
    type Me = CardId;
}

impl CardModel for SerpentForm {
    fn name(&self) -> &'static str {
        "Serpent Form"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::SerpentForm), 4).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::SerpentForm,
            eng.s
                .card(play.card)
                .var(Var::Power(PowerKind::SerpentForm)),
            Some(player),
        )
    }
}
