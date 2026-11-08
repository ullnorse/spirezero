//! `TheBombPower` (`Models.Powers/TheBombPower.cs`): an `Instanced` buff (`Counter`, the turns
//! left). Before the end of the owner's turn it counts down; on its last turn it deals its own
//! damage (set by the card, `SetDamage`) to every hittable enemy, unpowered, and removes itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerData, PowerInstanceType, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct TheBomb;

/// The instance's `DynamicVars.Damage` (`DamageVar(40m, ValueProp.Unpowered)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub damage: Decimal,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::TheBomb(data) => data,
        _ => unreachable!("a TheBomb instance holds TheBomb data"),
    }
}

impl PowerModel for TheBomb {
    fn name(&self) -> &'static str {
        "The Bomb"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::TheBomb(Data {
            damage: Decimal::from(40),
        })
    }
}

#[sts2_macros::hooks]
impl Hooks for TheBomb {
    type Me = PowerRef;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        if eng.s.power(me.owner, me.id).amount > 1 {
            return eng.decrement_power(me);
        }
        let damage = data(eng.s, me).damage;
        let targets = eng.hooked().hittable_enemies();
        eng.damage(
            &targets,
            damage,
            ValueProps::UNPOWERED,
            Some(me.owner),
            None,
        )?;
        eng.remove_power(me.owner, me.id)
    }
}
