//! `OrbitPower` (`Models.Powers/OrbitPower.cs`): an `Instanced` buff (`Counter`). Every 4 energy
//! the owner spends on cards returns `Amount` energy; one play can trigger it more than once.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, Engine, Flow};
use crate::content::powers::{PowerData, PowerInstanceType, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Orbit;

/// `_energyIncrement`.
const ENERGY_INCREMENT: i32 = 4;

/// `OrbitPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub energy_spent: i32,
    pub trigger_count: i32,
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Orbit(data) => data,
        _ => unreachable!("an Orbit instance holds Orbit data"),
    }
}

impl PowerModel for Orbit {
    fn name(&self) -> &'static str {
        "Orbit"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Orbit(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Orbit {
    type Me = PowerRef;

    fn after_energy_spent(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        amount: i32,
    ) -> Flow {
        // `card.Owner.Creature == Owner`: every card belongs to the player.
        if amount <= 0 {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.energy_spent += amount;
        let triggers = data.energy_spent / ENERGY_INCREMENT - data.trigger_count;
        if triggers > 0 {
            let energy = eng.s.power(me.owner, me.id).amount * triggers;
            eng.gain_energy(Decimal::from(energy))?;
            data_mut(eng.s, me).trigger_count += triggers;
        }
        Ok(())
    }
}
