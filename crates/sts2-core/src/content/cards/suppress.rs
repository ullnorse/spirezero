//! `Suppress` (`Models.Cards/Suppress.cs`): cost 0, 11 damage (17 upgraded) then 3 Weak (2 more upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Suppress;

#[sts2_macros::hooks]
impl Hooks for Suppress {
    type Me = CardId;
}

impl CardModel for Suppress {
    fn name(&self) -> &'static str {
        "Suppress"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Innate)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 11).upgrade(6),
            CardVar::new(Var::Power(PowerKind::Weak), 3).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Suppress needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Weak));
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
            PowerKind::Weak,
            amount,
            Some(player),
        )
    }
}
