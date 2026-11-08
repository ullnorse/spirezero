//! `Mangle` (`Models.Cards/Mangle.cs`): cost 3, 20 damage (26 upgraded), then the target loses 10 Strength (15 upgraded) this turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct Mangle;

#[sts2_macros::hooks]
impl Hooks for Mangle {
    type Me = CardId;
}

impl CardModel for Mangle {
    fn name(&self) -> &'static str {
        "Mangle"
    }

    fn cost(&self) -> i32 {
        3
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 20).upgrade(6),
            CardVar::new(Var::Named("StrengthLoss"), 10).upgrade(5),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Mangle needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Mangle,
            eng.s.card(play.card).var(Var::Named("StrengthLoss")),
            Some(player),
        )
    }
}
