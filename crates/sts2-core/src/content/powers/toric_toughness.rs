//! `ToricToughnessPower` (`Models.Powers/ToricToughnessPower.cs`): an `Instanced` buff
//! (`Counter`, the turns left). After the owner's block is cleared it gains its own block (set
//! by the card, `SetBlock`; unpowered) and counts down.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::{PowerData, PowerInstanceType, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct ToricToughness;

/// The instance's `DynamicVars.Block` (`BlockVar(0m, ValueProp.Unpowered)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub block: Decimal,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::ToricToughness(data) => data,
        _ => unreachable!("a ToricToughness instance holds ToricToughness data"),
    }
}

impl PowerModel for ToricToughness {
    fn name(&self) -> &'static str {
        "Toric Toughness"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::ToricToughness(Data {
            block: Decimal::ZERO,
        })
    }
}

#[sts2_macros::hooks]
impl Hooks for ToricToughness {
    type Me = PowerRef;

    fn after_block_cleared(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
    ) -> Flow {
        if creature != me.owner {
            return Ok(());
        }
        let block = data(eng.s, me).block;
        eng.gain_block(me.owner, block, ValueProps::UNPOWERED, None)?;
        eng.decrement_power(me)
    }
}
