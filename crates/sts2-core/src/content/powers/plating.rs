//! `PlatingPower` (`Models.Powers/PlatingPower.cs`): gains `Amount` Block at the start of its
//! owner's side's turn end (before end-of-turn damage), then loses one stack when the next turn
//! starts. An enemy owner also starts the combat with that Block and loses `Decrement` (the
//! number of players, 1 in single player) instead of one stack.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Plating;

impl PowerModel for Plating {
    fn name(&self) -> &'static str {
        "Plating"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

fn is_player(s: &CombatState, creature: CreatureId) -> bool {
    creature == s.player.creature
}

#[sts2_macros::hooks]
impl Hooks for Plating {
    type Me = PowerRef;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side != Side::Player || is_player(eng.s, me.owner) || eng.s.round > 1 {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        eng.gain_block(me.owner, Decimal::from(amount), ValueProps::UNPOWERED, None)?;
        Ok(())
    }

    fn before_side_turn_end_early(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.gain_block(me.owner, Decimal::from(amount), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        if is_player(eng.s, me.owner) {
            if eng.s.player.turn_number != 1 {
                eng.decrement_power(me)?;
            }
        } else if eng.s.round != 1 {
            // `Decrement` is the player count (1) for an enemy owner.
            eng.modify_power_amount(me.owner, me.id, Decimal::from(-1), None)?;
        }
        Ok(())
    }
}
