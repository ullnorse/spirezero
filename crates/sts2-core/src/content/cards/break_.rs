//! `Break` (`Models.Cards/Break.cs`): cost 1, 20 damage (30 upgraded) and 5 Vulnerable (7 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Break;

#[sts2_macros::hooks]
impl Hooks for Break {
    type Me = CardId;
}

impl CardModel for Break {
    fn name(&self) -> &'static str {
        "Break"
    }

    fn cost(&self) -> i32 {
        1
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 20).upgrade(10),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 5).upgrade(2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Break needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
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
            eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable)),
            Some(player),
        )
    }
}
