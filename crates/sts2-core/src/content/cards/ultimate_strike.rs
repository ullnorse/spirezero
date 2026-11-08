//! `UltimateStrike` (`Models.Cards/UltimateStrike.cs`): cost 1; 14 damage (20 upgraded). Tagged
//! `Strike` (read by no ported card).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct UltimateStrike;

#[sts2_macros::hooks]
impl Hooks for UltimateStrike {
    type Me = CardId;
}

impl CardModel for UltimateStrike {
    fn name(&self) -> &'static str {
        "Ultimate Strike"
    }

    fn cost(&self) -> i32 {
        1
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

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 14).upgrade(6)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("UltimateStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
