//! `ShrinkPower` (`Models.Powers/ShrinkPower.cs`): the owner's powered attacks deal 30% less.
//!
//! A negative amount is infinite (`ShrinkerBeetle` applies -1); a positive one loses 1 at the end
//! of the owner's side's turn through `PowerCmd.Decrement`, not `TickDownDuration`, so it has no
//! skip-tick (`ShrinkPower.cs:80-86`). Removed when its applier dies. A debuff that allows
//! negative amounts. `AfterApplied`/`AfterRemoved` only rescale the creature node (UI).

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Shrink;

/// `(100 - DamageDecrease) / 100` with `DamageDecrease = 30`.
const DAMAGE_FACTOR: Decimal = Decimal::new(7, 1);

/// `IsInfinite`: `Amount < 0`.
fn is_infinite(s: &CombatState, me: PowerRef) -> bool {
    s.power(me.owner, me.id).amount < 0
}

impl PowerModel for Shrink {
    fn name(&self) -> &'static str {
        "Shrink"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn allows_negative(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Shrink {
    type Me = PowerRef;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !is_infinite(eng.s, me) && participants.contains(&me.owner) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if !was_removal_prevented && Some(creature) == eng.s.power(me.owner, me.id).applier {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }

    fn modify_damage_multiplicative(
        &self,
        _s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.dealer != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        DAMAGE_FACTOR
    }
}
