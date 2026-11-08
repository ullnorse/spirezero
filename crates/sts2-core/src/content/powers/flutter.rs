//! `FlutterPower` (`Models.Powers/FlutterPower.cs`): the hovering `ThievingHopper` (`Counter`
//! buff) takes half damage from powered attacks (`DamageDecrease` 50). Each powered attack that
//! takes HP off it ticks it down; when it runs out the owner is stunned into the follow-up of
//! the move it last logged (`StateLog.Last().GetNextState`, a `MoveState`'s `FollowUpState`).
//! `IsHovering`, the trigger and the flash are animation only.

use crate::combat::hooks::{DamageEvent, DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::monsters::{self, machine::StateDef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

/// `DamageDecrease / 100m`.
const MULTIPLIER: Decimal = Decimal::new(5, 1);

/// Halves powered attack damage on the hovering `ThievingHopper`.
pub struct Flutter;

impl PowerModel for Flutter {
    fn name(&self) -> &'static str {
        "Flutter"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Flutter {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        _s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ONE;
        }
        MULTIPLIER
    }

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner || hit.result.unblocked == 0 || !hit.props.is_powered_attack() {
            return Ok(());
        }
        eng.decrement_power(me)?;
        let amount = eng
            .s
            .creature(me.owner)
            .powers
            .iter()
            .find(|p| p.id == me.id)
            .map_or(0, |p| p.amount);
        if amount > 0 {
            return Ok(());
        }
        let next = eng.s.creature(me.owner).monster.as_ref().map(|m| {
            let last = m.machine.log.last().expect("a monster has logged a move");
            match m.kind.moves().states[last] {
                StateDef::Move { follow_up, .. } => follow_up,
                _ => unreachable!("the state log holds moves only"),
            }
        });
        monsters::stun(eng, me.owner, next);
        Ok(())
    }
}
