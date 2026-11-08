//! `CrabRagePower` (`Models.Powers/CrabRagePower.cs`): a buff (`Single`) on both `KaiserCrabBoss`
//! arms. When another creature of its owner's side dies, the owner gains 6 Strength
//! (`PowerVar<StrengthPower>(6m)`) and 99 unpowered block (`BlockVar(99m, ValueProp.Unpowered)`),
//! and the power removes itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

/// `DynamicVars.Strength`.
const STRENGTH: i32 = 6;
/// `DynamicVars.Block`.
const BLOCK: i32 = 99;

/// The surviving crab arm's rage.
pub struct CrabRage;

impl PowerModel for CrabRage {
    fn name(&self) -> &'static str {
        "Crab Rage"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for CrabRage {
    type Me = PowerRef;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        _was_removal_prevented: bool,
    ) -> Flow {
        if creature == me.owner || eng.s.creature(creature).side != eng.s.creature(me.owner).side {
            return Ok(());
        }
        eng.apply_power(
            me.owner,
            PowerKind::Strength,
            Decimal::from(STRENGTH),
            Some(me.owner),
        )?;
        eng.gain_block(me.owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        eng.remove_power(me.owner, me.id)
    }
}
