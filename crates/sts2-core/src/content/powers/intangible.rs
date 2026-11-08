//! `IntangiblePower` (`Models.Powers/IntangiblePower.cs`): HP the owner would lose from a hit is
//! capped at 1 (after the Osty redirect) and the damage itself is capped at 1 (so block loss and
//! previews match); one stack is used up at the end of every enemy side's turn. `Flash` calls
//! are cosmetic.

use crate::combat::hooks::{DamageQuery, Hooks, HpLossQuery, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Intangible;

impl PowerModel for Intangible {
    fn name(&self) -> &'static str {
        "Intangible"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Intangible {
    type Me = PowerRef;

    fn modify_hp_lost_after_osty(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        // `CombatManager.Instance.IsInProgress`.
        if !s.in_progress || query.target != me.owner || amount < Decimal::ONE {
            return amount;
        }
        Decimal::ONE
    }

    fn modify_damage_cap(&self, _s: &CombatState, me: PowerRef, query: DamageQuery) -> Decimal {
        if query.target == Some(me.owner) {
            Decimal::ONE
        } else {
            Decimal::MAX
        }
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side == Side::Enemy {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
