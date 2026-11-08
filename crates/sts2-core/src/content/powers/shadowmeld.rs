//! `ShadowmeldPower` (`Models.Powers/ShadowmeldPower.cs`): the owner's Block gains are multiplied
//! by 2^`Amount`; the power is removed at the end of the owner's side's turn.

use crate::combat::hooks::{BlockQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Shadowmeld;

impl PowerModel for Shadowmeld {
    fn name(&self) -> &'static str {
        "Shadowmeld"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Shadowmeld {
    type Me = PowerRef;

    fn modify_block_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != me.owner {
            return Decimal::ONE;
        }
        // `(decimal)Math.Pow(2.0, Amount)`, exact for an integral `Amount`.
        let doubling = Decimal::from(2);
        (0..s.power(me.owner, me.id).amount).fold(Decimal::ONE, |factor, _| factor * doubling)
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
