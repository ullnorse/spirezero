//! `Abrasive` (`Models.Cards/Abrasive.cs`): cost 3, Sly; 1 Dexterity and 4 Thorns (6 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Abrasive;

#[sts2_macros::hooks]
impl Hooks for Abrasive {
    type Me = CardId;
}

impl CardModel for Abrasive {
    fn name(&self) -> &'static str {
        "Abrasive"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Sly)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Thorns), 4).upgrade(2),
            CardVar::new(Var::Power(PowerKind::Dexterity), 1),
        ];
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
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Thorns,
            eng.s.card(play.card).var(Var::Power(PowerKind::Thorns)),
            Some(player),
        )
    }
}
