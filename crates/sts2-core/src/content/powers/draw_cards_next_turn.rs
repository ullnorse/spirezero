//! `DrawCardsNextTurnPower` (`Models.Powers/DrawCardsNextTurnPower.cs`): a buff (`Counter`); the
//! owner's next hand draw is `Amount` cards larger, then it removes itself at its side's turn
//! start. Only counts once it was present at the turn start (`AmountOnTurnStart`).

use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct DrawCardsNextTurn;

impl PowerModel for DrawCardsNextTurn {
    fn name(&self) -> &'static str {
        "Draw Cards Next Turn"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for DrawCardsNextTurn {
    type Me = PowerRef;

    // `player != Owner.Player`: the only player owns it.
    fn modify_hand_draw(&self, s: &CombatState, me: PowerRef, count: Decimal) -> Decimal {
        let power = s.power(me.owner, me.id);
        if power.amount_on_turn_start == 0 {
            return count;
        }
        count + Decimal::from(power.amount)
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner)
            && eng.s.power(me.owner, me.id).amount_on_turn_start != 0
        {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
