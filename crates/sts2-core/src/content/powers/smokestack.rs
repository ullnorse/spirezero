//! `SmokestackPower` (`Models.Powers/SmokestackPower.cs`): a buff (`Counter`); whenever the owner
//! generates a Status card for combat, `Amount` unpowered damage to every hittable enemy.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CreatureId, Engine, Flow, ValueProps};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Smokestack;

impl PowerModel for Smokestack {
    fn name(&self) -> &'static str {
        "Smokestack"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Smokestack {
    type Me = PowerRef;

    fn after_card_generated_for_combat(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        card: CardId,
        creator: Option<CreatureId>,
    ) -> Flow {
        if eng.s.card(card).card_type() == CardType::Status && creator == Some(me.owner) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            let targets = eng.hooked().hittable_enemies();
            eng.damage(
                &targets,
                amount,
                ValueProps::UNPOWERED,
                Some(me.owner),
                None,
            )?;
        }
        Ok(())
    }
}
