//! `Snakebite` (`Models.Cards/Snakebite.cs`): cost 2, applies 7 Poison (10 upgraded) to one enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Snakebite;

#[sts2_macros::hooks]
impl Hooks for Snakebite {
    type Me = CardId;
}

impl CardModel for Snakebite {
    fn name(&self) -> &'static str {
        "Snakebite"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Retain)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Poison), 7).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Snakebite needs a target");
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Poison));
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Poison,
            amount,
            Some(player),
        )
    }
}
