//! `PoisonPower` (`Models.Powers/PoisonPower.cs`): at the start of its owner's side's turn the
//! owner takes `Amount` unblockable, unpowered damage from no one, then loses one stack; stacks
//! of `AccelerantPower` on the owner's opponents repeat that, never more often than `Amount`.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Poison;

impl PowerModel for Poison {
    fn name(&self) -> &'static str {
        "Poison"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

/// `TriggerCount`: `Math.Min(Amount, 1 + sum of the living opponents' Accelerant)`.
fn trigger_count(s: &CombatState, owner: CreatureId, amount: i32) -> i32 {
    let opponents = match s.creature(owner).side {
        Side::Player => Side::Enemy,
        Side::Enemy => Side::Player,
    };
    let accelerant: i32 = s
        .creatures_on(opponents)
        .into_iter()
        .map(|c| s.creature(c))
        .filter(|c| c.is_alive())
        .map(|c| c.power_amount(PowerKind::Accelerant))
        .sum();
    amount.min(1 + accelerant)
}

/// `PoisonPower.Trigger`: the Poison damage and the decrement, once per `TriggerCount`.
///
/// The C# loop keeps its `PowerModel` after the owner loses it. An owner that dies and comes
/// back within the damage (`WaterfallGiant`'s `SteamEruptionPower`) has had the Poison removed,
/// yet the later iterations still deal the detached instance's `Amount` and `PowerCmd.Decrement`
/// it, with every hook, as on any removed instance (`Creature::removed_powers`).
pub fn trigger(eng: &mut Engine<'_>, me: PowerRef) -> Flow {
    let iterations = trigger_count(eng.s, me.owner, eng.s.power(me.owner, me.id).amount);
    for _ in 0..iterations {
        // A dead owner has lost its powers; `CreatureCmd.Damage` on it does nothing, so the
        // remaining iterations have no effect.
        if eng.s.creature(me.owner).is_dead() {
            break;
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.damage(
            &[me.owner],
            amount,
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            None,
            None,
        )?;
        if eng.s.creature(me.owner).is_alive() {
            eng.decrement_power(me)?;
        }
    }
    Ok(())
}

#[sts2_macros::hooks]
impl Hooks for Poison {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            trigger(eng, me)?;
        }
        Ok(())
    }
}
