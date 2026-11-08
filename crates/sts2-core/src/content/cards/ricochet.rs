//! `Ricochet` (`Models.Cards/Ricochet.cs`): cost 2, Sly, 3 damage, 4 hits (5 upgraded) at random enemies.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Ricochet;

#[sts2_macros::hooks]
impl Hooks for Ricochet {
    type Me = CardId;
}

impl CardModel for Ricochet {
    fn name(&self) -> &'static str {
        "Ricochet"
    }

    fn cost(&self) -> i32 {
        2
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Sly)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 3),
            CardVar::new(Var::Repeat, 4).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = eng.s.card(play.card).count_var(Var::Repeat);
        let player = eng.s.player.creature;
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
