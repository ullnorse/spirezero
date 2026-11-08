//! `StockPower` (`Models.Powers/StockPower.cs`): a buff (`Counter`) on `Axebot`. When its owner
//! dies with `Amount` left, a fresh `Axebot` with `Amount - 1` stock takes the same slot, so the
//! owner's death does not end the combat.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::monsters::{Axebot, MonsterKind};
use crate::content::powers::{PowerModel, PowerType};

pub struct Stock;

impl PowerModel for Stock {
    fn name(&self) -> &'static str {
        "Stock"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Stock {
    type Me = PowerRef;

    /// `ShouldStopCombatFromEnding`: always, while the instance is on a creature.
    fn should_stop_combat_from_ending(&self, _s: &CombatState, _me: PowerRef) -> bool {
        true
    }

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        let amount = eng.s.power(me.owner, me.id).amount;
        if was_removal_prevented || creature != me.owner || amount <= 0 {
            return Ok(());
        }
        let slot = eng.s.creature(me.owner).slot;
        let replacement = Axebot {
            stock_override: Some(amount - 1),
        };
        eng.add_enemy(MonsterKind::Axebot(replacement), slot)?;
        Ok(())
    }
}
