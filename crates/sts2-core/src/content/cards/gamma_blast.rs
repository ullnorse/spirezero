//! `GammaBlast` (`Models.Cards/GammaBlast.cs`): Regent uncommon attack, cost 0 and 3 stars; deal 13 damage (18 upgraded), then apply 2 Weak and 2 Vulnerable.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct GammaBlast;

#[sts2_macros::hooks]
impl Hooks for GammaBlast {
    type Me = CardId;
}

impl CardModel for GammaBlast {
    fn name(&self) -> &'static str {
        "Gamma Blast"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn star_cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 13).upgrade(5),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 2),
            CardVar::new(Var::Power(PowerKind::Weak), 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("GammaBlast needs a target");
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
