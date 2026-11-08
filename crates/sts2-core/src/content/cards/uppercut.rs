//! `Uppercut` (`Models.Cards/Uppercut.cs`): cost 2, 13 damage, then 1 Weak and 1 Vulnerable (2 each upgraded).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct Uppercut;

#[sts2_macros::hooks]
impl Hooks for Uppercut {
    type Me = CardId;
}

impl CardModel for Uppercut {
    fn name(&self) -> &'static str {
        "Uppercut"
    }

    fn cost(&self) -> i32 {
        2
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
            CardVar::new(Var::Damage, 13),
            CardVar::new(Var::Named("Power"), 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Uppercut needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let amount = Decimal::from(eng.s.card(play.card).int_var(Var::Named("Power")));
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
            PowerKind::Weak,
            amount,
            Some(player),
        )?;
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Vulnerable,
            amount,
            Some(player),
        )
    }
}
