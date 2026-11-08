//! `KnockoutBlow` (`Models.Cards/KnockoutBlow.cs`): Regent uncommon attack, cost 3; deal 30 damage (38 upgraded), then gain 5 stars if a hit killed the target.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct KnockoutBlow;

#[sts2_macros::hooks]
impl Hooks for KnockoutBlow {
    type Me = CardId;
}

impl CardModel for KnockoutBlow {
    fn name(&self) -> &'static str {
        "Knockout Blow"
    }

    fn cost(&self) -> i32 {
        3
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
            CardVar::new(Var::Damage, 30).upgrade(8),
            CardVar::new(Var::Stars, 5),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("KnockoutBlow needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        let results = eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        if results.iter().any(|r| r.was_target_killed) {
            eng.gain_stars(eng.s.card(play.card).var(Var::Stars))?;
        }
        Ok(())
    }
}
