//! `Comet` (`Models.Cards/Comet.cs`): Regent rare attack, cost 0 and 5 stars; deal 33 damage (44 upgraded), then apply 3 Weak and 3 Vulnerable.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Comet;

#[sts2_macros::hooks]
impl Hooks for Comet {
    type Me = CardId;
}

impl CardModel for Comet {
    fn name(&self) -> &'static str {
        "Comet"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn star_cost(&self) -> i32 {
        5
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 33).upgrade(11),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 3),
            CardVar::new(Var::Power(PowerKind::Weak), 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Comet needs a target");
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
