//! `FallingStar` (`Models.Cards/FallingStar.cs`): Regent basic attack, cost 0 and 2 stars; deal 8
//! damage (12 upgraded), then apply 1 Weak and 1 Vulnerable.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct FallingStar;

#[sts2_macros::hooks]
impl Hooks for FallingStar {
    type Me = CardId;
}

impl CardModel for FallingStar {
    fn name(&self) -> &'static str {
        "Falling Star"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn star_cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Basic
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 8).upgrade(4),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1),
            CardVar::new(Var::Power(PowerKind::Weak), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("FallingStar needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        for kind in [PowerKind::Weak, PowerKind::Vulnerable] {
            let amount = eng.s.card(play.card).var(Var::Power(kind));
            eng.apply_power_from_card(Some(play.card), target, kind, amount, Some(player))?;
        }
        Ok(())
    }
}
