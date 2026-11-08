//! `SwordBoomerang` (`Models.Cards/SwordBoomerang.cs`): cost 1, 3 hits (4 upgraded) of 3 damage, each on a random enemy.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct SwordBoomerang;

#[sts2_macros::hooks]
impl Hooks for SwordBoomerang {
    type Me = CardId;
}

impl CardModel for SwordBoomerang {
    fn name(&self) -> &'static str {
        "Sword Boomerang"
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
        TargetType::RandomEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 3),
            CardVar::new(Var::Repeat, 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = eng.s.card(play.card).count_var(Var::Repeat);
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::RandomOpponents,
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
