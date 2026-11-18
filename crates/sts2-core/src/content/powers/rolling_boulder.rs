//! `RollingBoulderPower` (`Models.Powers/RollingBoulderPower.cs`): an `Instanced` buff
//! (`Counter`). At the start of the owner's turn it deals `Amount` unpowered damage to every
//! hittable enemy, then `Amount` grows by 5 (`SetAmount`, no hooks).
//!
//! In test mode (`TestMode.IsOn`) the damage is one batched `CreatureCmd.Damage` on all hittable
//! enemies: every HP loss first, then the after-hooks and deaths. In the real game
//! `NRollingBoulderVfx` emits `HitCreature` as the boulder reaches each enemy and the handler
//! makes one `DoDamage` call for that enemy alone, so one enemy's after-hooks and death (a Kaiser
//! crab arm's `CrabRage`) run before the next enemy is hit. The VFX hits an enemy once the boulder
//! (rolling left to right) passes the left edge of its visuals; that position comes from the
//! scene, not the code, so the order used is the order of `HittableEnemies` at the turn start
//! (also the order within one frame). Dead enemies are skipped by `Damage` itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Event, Flow, ValueProps};
use crate::content::powers::{PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct RollingBoulder;

/// `DamageVar(5m, ValueProp.Unpowered)`: the growth per turn.
const INCREMENT: i32 = 5;

impl PowerModel for RollingBoulder {
    fn name(&self) -> &'static str {
        "Rolling Boulder"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }
}

#[sts2_macros::hooks]
impl Hooks for RollingBoulder {
    type Me = PowerRef;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player != Owner.Player`: the only player owns it.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        let targets = eng.hooked().hittable_enemies();
        let amount = Decimal::from(amount);
        if eng.s.test_mode {
            eng.damage(
                &targets,
                amount,
                ValueProps::UNPOWERED,
                Some(me.owner),
                None,
            )?;
        } else {
            for &target in &targets {
                eng.damage(
                    &[target],
                    amount,
                    ValueProps::UNPOWERED,
                    Some(me.owner),
                    None,
                )?;
            }
        }
        // SetAmount(Amount + Damage), clamped; the instance may have left its owner.
        let Some(power) = eng
            .s
            .creature_mut(me.owner)
            .powers
            .iter_mut()
            .find(|p| p.id == me.id)
        else {
            return Ok(());
        };
        let new = (power.amount + INCREMENT).clamp(-999_999_999, 999_999_999);
        if new != power.amount {
            power.amount = new;
            eng.emit(Event::PowerChanged {
                creature: me.owner,
                kind: PowerKind::RollingBoulder,
                amount: new,
            });
        }
        Ok(())
    }
}
