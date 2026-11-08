//! `Thunderclap` (`Models.Cards/Thunderclap.cs`): cost 1, 4 damage (7 upgraded) and 1 Vulnerable to all enemies.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Thunderclap;

#[sts2_macros::hooks]
impl Hooks for Thunderclap {
    type Me = CardId;
}

impl CardModel for Thunderclap {
    fn name(&self) -> &'static str {
        "Thunderclap"
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
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 4).upgrade(3),
            CardVar::new(Var::Power(PowerKind::Vulnerable), 1),
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
        let targets = eng.hooked().hittable_enemies();
        eng.apply_power_to_all_from_card(
            Some(play.card),
            &targets,
            PowerKind::Vulnerable,
            eng.s.card(play.card).var(Var::Power(PowerKind::Vulnerable)),
            Some(player),
        )
    }
}
