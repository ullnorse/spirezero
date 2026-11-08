//! `CollisionCourse` (`Models.Cards/CollisionCourse.cs`): Regent common attack, cost 0; deal 10
//! damage (14 upgraded), then add a `Debris` to the hand.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct CollisionCourse;

#[sts2_macros::hooks]
impl Hooks for CollisionCourse {
    type Me = CardId;
}

impl CardModel for CollisionCourse {
    fn name(&self) -> &'static str {
        "Collision Course"
    }

    fn cost(&self) -> i32 {
        0
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("CollisionCourse needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let debris = eng.create_card(CardKind::Debris);
        eng.add_generated_card_to_combat(debris, PileType::Hand, Some(player))?;
        Ok(())
    }
}
