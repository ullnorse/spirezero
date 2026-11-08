//! `RipAndTear` (`Models.Cards/RipAndTear.cs`): Event attack, cost 1; 7 damage (9 upgraded)
//! twice, each hit at a random enemy.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct RipAndTear;

#[sts2_macros::hooks]
impl Hooks for RipAndTear {
    type Me = CardId;
}

impl CardModel for RipAndTear {
    fn name(&self) -> &'static str {
        "Rip and Tear"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::RandomEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 7).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::RandomOpponents,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(2),
        )?;
        Ok(())
    }
}
