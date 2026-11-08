//! `JuggernautPower` (`Models.Powers/JuggernautPower.cs`): whenever the owner gains Block, a
//! random hittable enemy (`CombatTargets` stream) takes `Amount` unpowered damage.

use crate::combat::ValueProps;
use crate::combat::hooks::{BlockQuery, Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Juggernaut;

impl PowerModel for Juggernaut {
    fn name(&self) -> &'static str {
        "Juggernaut"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Juggernaut {
    type Me = PowerRef;

    fn after_block_gained(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        query: BlockQuery,
        amount: Decimal,
    ) -> Flow {
        if amount <= Decimal::ZERO || query.target != me.owner {
            return Ok(());
        }
        let enemies = eng.hooked().hittable_enemies();
        let Some(target) = eng.random_target(&enemies) else {
            return Ok(());
        };
        let damage = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.damage(
            &[target],
            damage,
            ValueProps::UNPOWERED,
            Some(me.owner),
            None,
        )?;
        Ok(())
    }
}
