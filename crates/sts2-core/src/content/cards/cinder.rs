//! `Cinder` (`Models.Cards/Cinder.cs`): 18 damage (24 upgraded), then exhaust a random card of the
//! hand (`Rng.CombatCardSelection`).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::rng::RunRngType;

pub struct Cinder;

#[sts2_macros::hooks]
impl Hooks for Cinder {
    type Me = CardId;
}

impl CardModel for Cinder {
    fn name(&self) -> &'static str {
        "Cinder"
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
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 18).upgrade(6)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Cinder needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let hand = eng.s.player.hand.to_vec();
        let picked = eng.s.rng[RunRngType::CombatCardSelection]
            .next_item(&hand)
            .copied();
        if let Some(card) = picked {
            eng.exhaust(card, false)?;
        }
        Ok(())
    }
}
