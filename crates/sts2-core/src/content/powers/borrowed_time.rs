//! `BorrowedTimePower` (`Models.Powers/BorrowedTimePower.cs`): the owner's cards cost `Amount`
//! more; removed after the owner's side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct BorrowedTime;

impl PowerModel for BorrowedTime {
    fn name(&self) -> &'static str {
        "Borrowed Time"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for BorrowedTime {
    type Me = PowerRef;

    fn try_modify_energy_cost_in_combat(
        &self,
        s: &CombatState,
        me: PowerRef,
        _card: CardId,
        cost: Decimal,
    ) -> Option<Decimal> {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if s.player.creature != me.owner {
            return None;
        }
        Some(cost + Decimal::from(s.power(me.owner, me.id).amount))
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
