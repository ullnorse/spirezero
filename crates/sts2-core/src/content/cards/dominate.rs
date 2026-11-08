//! `Dominate` (`Models.Cards/Dominate.cs`): cost 1, Exhaust, 1 Vulnerable (2 upgraded), then Strength equal to the target's Vulnerable.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Dominate;

#[sts2_macros::hooks]
impl Hooks for Dominate {
    type Me = CardId;
}

impl CardModel for Dominate {
    fn name(&self) -> &'static str {
        "Dominate"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1).upgrade(1),
            CardVar::new(Var::Named("StrengthPerVulnerable"), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Dominate needs a target");
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable)),
            Some(player),
        )?;
        let strength = eng.s.creature(target).power_amount(PowerKind::Vulnerable);
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Strength,
            Decimal::from(strength),
            Some(player),
        )
    }
}
