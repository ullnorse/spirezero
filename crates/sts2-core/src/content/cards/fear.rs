//! `Fear` (`Models.Cards/Fear.cs`): cost 1, Ethereal, 7 damage (8 upgraded) and 1 Vulnerable (2 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;

pub struct Fear;

#[sts2_macros::hooks]
impl Hooks for Fear {
    type Me = CardId;
}

impl CardModel for Fear {
    fn name(&self) -> &'static str {
        "Fear"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Ethereal)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 7).upgrade(1),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Fear needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let vulnerable = eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable));
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            vulnerable,
            Some(player),
        )
    }
}
