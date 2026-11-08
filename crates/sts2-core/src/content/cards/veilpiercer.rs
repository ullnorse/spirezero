//! `Veilpiercer` (`Models.Cards/Veilpiercer.cs`): cost 1, 10 damage (13 upgraded), then applies
//! 1 `VeilpiercerPower`. The Ethereal in the inventory is a hover tip, not a canonical keyword.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Veilpiercer;

#[sts2_macros::hooks]
impl Hooks for Veilpiercer {
    type Me = CardId;
}

impl CardModel for Veilpiercer {
    fn name(&self) -> &'static str {
        "Veilpiercer"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Veilpiercer needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.apply_power(
            player,
            PowerKind::Veilpiercer,
            Decimal::from(1),
            Some(player),
        )
    }
}
