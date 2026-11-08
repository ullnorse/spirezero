//! `InfernoPower` (`Models.Powers/InfernoPower.cs`): at the start of the owner's turn it loses
//! `SelfDamage` HP (Unblockable, Unpowered; `Inferno` raises it by 1 per play); whenever the
//! owner loses HP on its side's turn every hittable enemy takes `Amount` unpowered damage.

use crate::combat::ValueProps;
use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Inferno;

/// `InfernoPower`'s `CanonicalVars`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `DamageVar("SelfDamage", 0m, Unblockable | Unpowered)`'s base value.
    pub self_damage: i32,
}

/// `InfernoPower.IncrementSelfDamage`.
pub fn increment_self_damage(s: &mut CombatState, me: PowerRef) {
    let PowerData::Inferno(data) = s.power_data_mut(me.id) else {
        unreachable!("an Inferno instance holds Inferno data");
    };
    data.self_damage += 1;
}

impl PowerModel for Inferno {
    fn name(&self) -> &'static str {
        "Inferno"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Inferno(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Inferno {
    type Me = PowerRef;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player owns the power if its owner is the player.
        if me.owner != eng.s.player.creature {
            return Ok(());
        }
        let PowerData::Inferno(data) = *eng.s.power_data(me.id) else {
            unreachable!("an Inferno instance holds Inferno data");
        };
        eng.damage(
            &[me.owner],
            Decimal::from(data.self_damage),
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            Some(me.owner),
            None,
        )?;
        Ok(())
    }

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner
            || hit.result.unblocked <= 0
            || eng.s.side != eng.s.creature(me.owner).side
        {
            return Ok(());
        }
        let enemies = eng.hooked().hittable_enemies();
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.damage(
            &enemies,
            amount,
            ValueProps::UNPOWERED,
            Some(me.owner),
            None,
        )?;
        Ok(())
    }
}
