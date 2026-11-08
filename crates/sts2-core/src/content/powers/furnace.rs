//! `FurnacePower` (`Models.Powers/FurnacePower.cs`): a buff (`Counter`); Forge `Amount` at the
//! start of the owner's side's turn.

use crate::combat::hooks::{Hooks, Listener, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Furnace;

impl PowerModel for Furnace {
    fn name(&self) -> &'static str {
        "Furnace"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Furnace {
    type Me = PowerRef;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.forge(
                amount,
                Some(Listener::Power {
                    owner: me.owner,
                    power: me.id,
                }),
            )?;
        }
        Ok(())
    }
}
