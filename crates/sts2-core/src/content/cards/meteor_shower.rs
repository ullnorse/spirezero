//! `MeteorShower` (`Models.Cards/MeteorShower.cs`): Regent ancient attack, cost 0 and 2 stars; deal 14 damage (21 upgraded) to all enemies, then apply 2 Weak and 2 Vulnerable to each.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct MeteorShower;

#[sts2_macros::hooks]
impl Hooks for MeteorShower {
    type Me = CardId;
}

impl CardModel for MeteorShower {
    fn name(&self) -> &'static str {
        "Meteor Shower"
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
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 14).upgrade(7),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 2),
            CardVar::new(Var::Power(PowerKind::Weak), 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        for kind in [PowerKind::Weak, PowerKind::Vulnerable] {
            let amount = eng.s.card(play.card).var(Var::Power(kind));
            let enemies = eng.hooked().hittable_enemies();
            eng.apply_power_to_all_from_card(
                Some(play.card),
                &enemies,
                kind,
                amount,
                Some(player),
            )?;
        }
        Ok(())
    }
}
