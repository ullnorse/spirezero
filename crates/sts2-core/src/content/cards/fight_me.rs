//! `FightMe` (`Models.Cards/FightMe.cs`): cost 2, 5 damage (6 upgraded) twice, then 3 Strength (4 upgraded) for you and 1 for the target.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct FightMe;

#[sts2_macros::hooks]
impl Hooks for FightMe {
    type Me = CardId;
}

impl CardModel for FightMe {
    fn name(&self) -> &'static str {
        "Fight Me!"
    }

    fn cost(&self) -> i32 {
        2
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
            CardVar::new(Var::Damage, 5).upgrade(1),
            CardVar::new(Var::Repeat, 2),
            CardVar::new(Var::Power(PowerKind::Strength), 3).upgrade(1),
            CardVar::new(Var::Named("EnemyStrength"), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("FightMe needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(eng.s.card(play.card).count_var(Var::Repeat)),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::Strength,
            eng.s.card(play.card).var(Var::Power(PowerKind::Strength)),
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Strength,
            eng.s.card(play.card).var(Var::Named("EnemyStrength")),
            Some(player),
        )
    }
}
