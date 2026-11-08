//! `CoolantPower` (`Models.Powers/CoolantPower.cs`): at the start of the owner's side's turn,
//! `Amount` block (`Unpowered`) per distinct kind of orb in the queue.

use std::collections::HashSet;

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Coolant;

impl PowerModel for Coolant {
    fn name(&self) -> &'static str {
        "Coolant"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Coolant {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            // `group orb by orb.Id`: one group per orb kind.
            let kinds: HashSet<_> = eng.s.player.orbs.iter().map(|o| o.kind).collect();
            let kinds = i32::try_from(kinds.len()).expect("at most 10 orbs");
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.gain_block(
                me.owner,
                Decimal::from(kinds * amount),
                ValueProps::UNPOWERED,
                None,
            )?;
        }
        Ok(())
    }
}
