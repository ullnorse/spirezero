//! `HangPower` (`Models.Powers/HangPower.cs`): a counter debuff; the owner takes `Amount` times
//! the damage of an attack from the `Hang` card.

use crate::combat::CombatState;
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Hang;

impl PowerModel for Hang {
    fn name(&self) -> &'static str {
        "Hang"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }
}

#[sts2_macros::hooks]
impl Hooks for Hang {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != Some(me.owner) {
            return Decimal::ONE;
        }
        // `cardSource is Hang`.
        if !query
            .card
            .is_some_and(|card| s.card(card).kind == CardKind::Hang)
        {
            return Decimal::ONE;
        }
        Decimal::from(s.power(me.owner, me.id).amount)
    }
}
