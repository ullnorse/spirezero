//! `BlightStrike` (`Models.Cards/BlightStrike.cs`): cost 1, 8 damage (10 upgraded), then applies Doom equal to the damage dealt.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct BlightStrike;

#[sts2_macros::hooks]
impl Hooks for BlightStrike {
    type Me = CardId;
}

impl CardModel for BlightStrike {
    fn name(&self) -> &'static str {
        "Blight Strike"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 8).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("BlightStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        let results = eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // `Results.SelectMany(..).Sum(r => r.TotalDamage)`, `BlockedDamage + UnblockedDamage`.
        let total: i32 = results.iter().map(|r| r.blocked + r.unblocked).sum();
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Doom,
            Decimal::from(total),
            Some(player),
        )
    }
}
