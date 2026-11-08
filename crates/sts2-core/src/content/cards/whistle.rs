//! `Whistle` (`Models.Cards/Whistle.cs`): Ancient (`TanxsWhistle`); cost 2, Exhaust, 33 damage
//! (44 upgraded), then the target is stunned (`CreatureCmd.Stun`, no next move: it repeats the
//! last move it logged).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::monsters;

pub struct Whistle;

#[sts2_macros::hooks]
impl Hooks for Whistle {
    type Me = CardId;
}

impl CardModel for Whistle {
    fn name(&self) -> &'static str {
        "Whistle"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 33).upgrade(11)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Whistle needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        monsters::stun(eng, target, None);
        Ok(())
    }
}
