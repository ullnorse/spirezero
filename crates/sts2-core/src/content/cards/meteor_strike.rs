//! `MeteorStrike` (`Models.Cards/MeteorStrike.cs`): cost 5, a Strike, 24 damage (30 upgraded), then channel 3 Plasma.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct MeteorStrike;

#[sts2_macros::hooks]
impl Hooks for MeteorStrike {
    type Me = CardId;
}

impl CardModel for MeteorStrike {
    fn name(&self) -> &'static str {
        "Meteor Strike"
    }

    fn cost(&self) -> i32 {
        5
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

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 24).upgrade(6)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("MeteorStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        for _ in 0..3 {
            eng.channel(OrbKind::Plasma)?;
        }
        Ok(())
    }
}
