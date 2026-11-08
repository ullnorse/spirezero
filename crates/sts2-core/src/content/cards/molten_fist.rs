//! `MoltenFist` (`Models.Cards/MoltenFist.cs`): cost 1, Exhaust, 10 damage (14 upgraded), then doubles the target's Vulnerable.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct MoltenFist;

#[sts2_macros::hooks]
impl Hooks for MoltenFist {
    type Me = CardId;
}

impl CardModel for MoltenFist {
    fn name(&self) -> &'static str {
        "Molten Fist"
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(4)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("MoltenFist needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let creature = eng.s.creature(target);
        let stacks = if creature.is_alive() {
            creature.power_amount(PowerKind::Vulnerable)
        } else {
            0
        };
        if stacks > 0 {
            eng.apply_power_from_card(
                Some(play.card),
                target,
                PowerKind::Vulnerable,
                Decimal::from(stacks),
                Some(player),
            )?;
        }
        Ok(())
    }
}
