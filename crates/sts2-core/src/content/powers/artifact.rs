//! `ArtifactPower` (`Models.Powers/ArtifactPower.cs`): the owner negates the next debuffs
//! applied to it; each negated debuff uses one stack. `ShouldScaleInMultiplayer` only matters
//! with several players.

use crate::combat::hooks::{Hooks, PowerQuery, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Artifact;

impl PowerModel for Artifact {
    fn name(&self) -> &'static str {
        "Artifact"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Artifact {
    type Me = PowerRef;

    fn try_modify_power_amount_received(
        &self,
        _s: &CombatState,
        me: PowerRef,
        power: PowerQuery,
        amount: Decimal,
    ) -> Option<Decimal> {
        // `IsVisible` of the canonical power is true for every ported power.
        if power.target != me.owner || power.kind.type_for_amount(amount) != PowerType::Debuff {
            return None;
        }
        Some(Decimal::ZERO)
    }

    fn after_modifying_power_amount_received(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _power: PowerQuery,
    ) -> Flow {
        eng.decrement_power(me)
    }
}
