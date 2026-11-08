//! `CalcifyPower` (`Models.Powers/CalcifyPower.cs`): the owner's Osty deals `Amount` more damage
//! with powered attacks.

use crate::combat::CombatState;
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::content::monsters::Monster;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Calcify;

impl PowerModel for Calcify {
    fn name(&self) -> &'static str {
        "Calcify"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Calcify {
    type Me = PowerRef;

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        let Some(dealer) = query.dealer.map(|d| s.creature(d)) else {
            return Decimal::ZERO;
        };
        if !dealer.monster.as_ref().is_some_and(Monster::is_osty) {
            return Decimal::ZERO;
        }
        // `Owner != dealer.PetOwner?.Creature`.
        if dealer.pet_owner != Some(me.owner) || !query.props.is_powered_attack() {
            return Decimal::ZERO;
        }
        Decimal::from(s.power(me.owner, me.id).amount)
    }
}
