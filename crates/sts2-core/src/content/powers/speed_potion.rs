//! `SpeedPotionPower` (`Models.Powers/SpeedPotionPower.cs`, a `TemporaryDexterityPower`): the
//! owner gains `Amount` Dexterity until the end of its side's turn. Applying it applies
//! `+amount` Dexterity; a stack applies `+amount` more; at the end of the owner's side's turn it
//! removes itself and takes the Dexterity back.

use super::temporary::Temporary;
use super::temporary_dexterity;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SpeedPotion;

/// `InternallyAppliedPower` and `IsPositive`.
const TEMPORARY: Temporary = temporary_dexterity::UP;

impl PowerModel for SpeedPotion {
    fn name(&self) -> &'static str {
        "Speed Potion"
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
impl Hooks for SpeedPotion {
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
