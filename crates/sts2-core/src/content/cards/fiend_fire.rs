//! `FiendFire` (`Models.Cards/FiendFire.cs`): exhaust the whole hand, then hit once per card
//! exhausted for 7 damage (10 upgraded). Exhaust.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct FiendFire;

#[sts2_macros::hooks]
impl Hooks for FiendFire {
    type Me = CardId;
}

impl CardModel for FiendFire {
    fn name(&self) -> &'static str {
        "Fiend Fire"
    }

    fn cost(&self) -> i32 {
        2
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 7).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("FiendFire needs a target");
        let hand = eng.s.player.hand.to_vec();
        for &card in &hand {
            eng.exhaust(card, false)?;
        }
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        let hits = u32::try_from(hand.len()).expect("a hand fits u32");
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
