//! `AccuracyPower` (`Models.Powers/AccuracyPower.cs`): a counter buff; `Amount` extra damage on
//! its owner's powered attacks from Shiv cards (`CardTag.Shiv`).

use crate::combat::CombatState;
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Accuracy;

impl PowerModel for Accuracy {
    fn name(&self) -> &'static str {
        "Accuracy"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Accuracy {
    type Me = PowerRef;

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.dealer != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ZERO;
        }
        match query.card {
            Some(card) if s.card(card).kind.is_shiv() => {
                Decimal::from(s.power(me.owner, me.id).amount)
            }
            _ => Decimal::ZERO,
        }
    }
}
