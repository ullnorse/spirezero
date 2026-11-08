//! `PiercingWail` (`Models.Cards/PiercingWail.cs`): cost 1, Exhaust; every enemy loses 6 Strength this turn (8 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct PiercingWail;

#[sts2_macros::hooks]
impl Hooks for PiercingWail {
    type Me = CardId;
}

impl CardModel for PiercingWail {
    fn name(&self) -> &'static str {
        "Piercing Wail"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("StrengthLoss"), 6).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let amount = eng.s.card(play.card).var(Var::Named("StrengthLoss"));
        let targets = eng.hooked().hittable_enemies();
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &targets,
            PowerKind::PiercingWail,
            amount,
            Some(player),
        )
    }
}
