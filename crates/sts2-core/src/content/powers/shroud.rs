//! `ShroudPower` (`Models.Powers/ShroudPower.cs`): whenever the owner applies Doom (any change of
//! a Doom power it applied), gain `Amount` unpowered Block.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Shroud;

impl PowerModel for Shroud {
    fn name(&self) -> &'static str {
        "Shroud"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Shroud {
    type Me = PowerRef;

    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        _amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        // The changed instance may already be off its owner (an earlier listener killed it).
        let is_doom = eng.s.power(power.owner, power.id).kind == PowerKind::Doom;
        if applier != Some(me.owner) || !is_doom {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}
