//! `Assassinate` (`Models.Cards/Assassinate.cs`): cost 0, 10 damage (13 upgraded) then 1 Vulnerable (1 more upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Assassinate;

#[sts2_macros::hooks]
impl Hooks for Assassinate {
    type Me = CardId;
}

impl CardModel for Assassinate {
    fn name(&self) -> &'static str {
        "Assassinate"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE
            .with(CardKeyword::Innate)
            .with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 10).upgrade(3),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Assassinate needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable));
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            amount,
            Some(player),
        )
    }
}
