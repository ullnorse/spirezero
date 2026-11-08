//! `CrimsonMantlePower` (`Models.Powers/CrimsonMantlePower.cs`): at the start of the owner's turn
//! it loses `SelfDamage` HP (Unblockable, Unpowered; `CrimsonMantle` raises it by 1 per play),
//! then gains `Amount` unpowered Block.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, Engine, Flow};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct CrimsonMantle;

/// `CrimsonMantlePower`'s `CanonicalVars`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `DamageVar("SelfDamage", 0m, Unblockable | Unpowered)`'s base value.
    pub self_damage: i32,
}

/// `CrimsonMantlePower.IncrementSelfDamage`.
pub fn increment_self_damage(s: &mut CombatState, me: PowerRef) {
    let PowerData::CrimsonMantle(data) = s.power_data_mut(me.id) else {
        unreachable!("a CrimsonMantle instance holds CrimsonMantle data");
    };
    data.self_damage += 1;
}

impl PowerModel for CrimsonMantle {
    fn name(&self) -> &'static str {
        "Crimson Mantle"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::CrimsonMantle(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for CrimsonMantle {
    type Me = PowerRef;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        // `player == Owner.Player`: the only player owns the power if its owner is the player.
        if me.owner != eng.s.player.creature {
            return Ok(());
        }
        let PowerData::CrimsonMantle(data) = *eng.s.power_data(me.id) else {
            unreachable!("a CrimsonMantle instance holds CrimsonMantle data");
        };
        // `Amount` is read before the damage: a lethal hit removes the owner's powers.
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.damage(
            &[me.owner],
            Decimal::from(data.self_damage),
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            Some(me.owner),
            None,
        )?;
        eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
