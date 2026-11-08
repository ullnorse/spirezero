//! `SummonNextTurnPower` (`Models.Powers/SummonNextTurnPower.cs`): after the owner's next player
//! turn starts, summon Osty for `Amount` and remove itself.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SummonNextTurn;

impl PowerModel for SummonNextTurn {
    fn name(&self) -> &'static str {
        "Summon Next Turn"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SummonNextTurn {
    type Me = PowerRef;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: PowerRef) -> Flow {
        let power = eng.s.power(me.owner, me.id);
        if power.amount_on_turn_start == 0 {
            return Ok(());
        }
        let amount = Decimal::from(power.amount);
        eng.summon_osty(amount)?;
        eng.remove_power(me.owner, me.id)
    }
}
