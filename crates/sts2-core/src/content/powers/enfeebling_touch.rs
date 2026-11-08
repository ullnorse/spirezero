//! `EnfeeblingTouchPower` (`Models.Powers/EnfeeblingTouchPower.cs`, a `TemporaryStrengthPower`
//! with `IsPositive` false): the owner loses `Amount` Strength until the end of its side's turn.
//! Applying it applies `-amount` Strength; a stack applies `-amount` more; at the end of the
//! owner's side's turn it removes itself and gives the Strength back.

use super::temporary::Temporary;
use super::temporary_strength;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct EnfeeblingTouch;

/// `InternallyAppliedPower` and `IsPositive`.
const TEMPORARY: Temporary = temporary_strength::DOWN;

impl PowerModel for EnfeeblingTouch {
    fn name(&self) -> &'static str {
        "Enfeebling Touch"
    }

    fn power_type(&self) -> PowerType {
        TEMPORARY.power_type()
    }

    fn internally_applied_power(&self) -> Option<PowerKind> {
        Some(TEMPORARY.stat)
    }

    fn before_applied(
        &self,
        eng: &mut Engine<'_>,
        target: CreatureId,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        TEMPORARY.before_applied(eng, target, amount, applier)
    }
}

#[sts2_macros::hooks]
impl Hooks for EnfeeblingTouch {
    type Me = PowerRef;

    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        TEMPORARY.after_power_amount_changed(eng, me, power, amount, applier)
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        TEMPORARY.after_side_turn_end(eng, me, participants)
    }
}
