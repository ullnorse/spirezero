//! `ConquerorPower` (`Models.Powers/ConquerorPower.cs`): a debuff (`Counter`); the owner takes
//! double damage from powered attacks of a Sovereign Blade. Loses a stack at the end of its
//! side's turn.

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Conqueror;

impl PowerModel for Conqueror {
    fn name(&self) -> &'static str {
        "Conqueror"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Conqueror {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        let from_blade = query
            .card
            .is_some_and(|c| s.card(c).kind == CardKind::SovereignBlade);
        if from_blade && query.props.is_powered_attack() && query.target == Some(me.owner) {
            Decimal::from(2)
        } else {
            Decimal::ONE
        }
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.decrement_power(me)?;
        }
        Ok(())
    }
}
