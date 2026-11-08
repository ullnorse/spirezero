//! `SlipperyPower` (`Models.Powers/SlipperyPower.cs`): HP the owner would lose from a hit is
//! capped at 1 (after the Osty redirect); each hit that costs it HP uses up one stack.
//! `ShouldScaleInMultiplayer` only matters with several players.

use crate::combat::hooks::{DamageEvent, Hooks, HpLossQuery, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Slippery;

impl PowerModel for Slippery {
    fn name(&self) -> &'static str {
        "Slippery"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Slippery {
    type Me = PowerRef;

    fn modify_hp_lost_after_osty(
        &self,
        _s: &CombatState,
        me: PowerRef,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        if query.target != me.owner || amount < Decimal::ONE {
            return amount;
        }
        Decimal::ONE
    }

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target == me.owner && hit.result.unblocked >= 1 {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
