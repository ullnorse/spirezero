//! `RollingBoulderPower` (`Models.Powers/RollingBoulderPower.cs`): an `Instanced` buff
//! (`Counter`). At the start of the owner's turn it deals `Amount` unpowered damage to every
//! hittable enemy, then `Amount` grows by 5 (`SetAmount`, no hooks).
//!
//! In test mode the damage is one `CreatureCmd.Damage` on all hittable enemies; the real game
//! damages each enemy in its own call as the boulder's VFX reaches it. `docs/specs/test-mode.md`
//! classifies the branch as cosmetic; the test-mode branch is ported.

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
        eng.damage(
            &targets,
            Decimal::from(amount),
            ValueProps::UNPOWERED,
            Some(me.owner),
            None,
        )?;
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
