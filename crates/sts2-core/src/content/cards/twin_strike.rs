//! `TwinStrike` (`Models.Cards/TwinStrike.cs`): cost 1, Strike; two hits of 5 damage (7 upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct TwinStrike;

#[sts2_macros::hooks]
impl Hooks for TwinStrike {
    type Me = CardId;
}

impl CardModel for TwinStrike {
    fn name(&self) -> &'static str {
        "Twin Strike"
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
        TargetType::AnyEnemy
    }

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 5).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("TwinStrike needs a target");
        let player = eng.s.player.creature;
        let damage = eng.s.card(play.card).var(Var::Damage);
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(2),
        )?;
        Ok(())
    }
}
