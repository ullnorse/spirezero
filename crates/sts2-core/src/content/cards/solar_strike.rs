//! `SolarStrike` (`Models.Cards/SolarStrike.cs`): Regent common attack, cost 1; deal 9 damage (10 upgraded), then gain 1 star (2 upgraded). Tagged Strike.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct SolarStrike;

#[sts2_macros::hooks]
impl Hooks for SolarStrike {
    type Me = CardId;
}

impl CardModel for SolarStrike {
    fn name(&self) -> &'static str {
        "Solar Strike"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn is_strike(&self) -> bool {
        true
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 9).upgrade(1),
            CardVar::new(Var::Stars, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("SolarStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.gain_stars(eng.s.card(play.card).var(Var::Stars))?;
        Ok(())
    }
}
