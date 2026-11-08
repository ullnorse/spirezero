//! `TrackingPower` (`Models.Powers/TrackingPower.cs`): a buff (`Counter`); the owner's (and its
//! pets') powered card attacks deal `1 + Amount / 100` times damage to targets with Weak.

use crate::combat::CombatState;
use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Tracking;

impl PowerModel for Tracking {
    fn name(&self) -> &'static str {
        "Tracking"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Tracking {
    type Me = PowerRef;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        let from_owner = query.dealer.is_some_and(|d| {
            d == me.owner || (s.player.creature == me.owner && s.player.pets.contains(&d))
        });
        if !query.props.is_powered_attack() || query.card.is_none() || !from_owner {
            return Decimal::ONE;
        }
        if !query
            .target
            .is_some_and(|t| s.creature(t).has_power(PowerKind::Weak))
        {
            return Decimal::ONE;
        }
        // `1m + (decimal)Amount / 100m`.
        Decimal::new(100 + i128::from(s.power(me.owner, me.id).amount), 2)
    }
}
