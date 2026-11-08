//! `ThornsPower` (`Models.Powers/ThornsPower.cs`): before the owner receives damage from a
//! dealer that is a powered attack (or comes from `Omnislice`), the dealer takes `Amount`
//! unpowered damage from the owner.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, HpLossQuery, PowerRef};
use crate::combat::{Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Thorns;

impl PowerModel for Thorns {
    fn name(&self) -> &'static str {
        "Thorns"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Thorns {
    type Me = PowerRef;

    fn before_damage_received(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        query: HpLossQuery,
        _amount: Decimal,
    ) -> Flow {
        let Some(dealer) = query.dealer else {
            return Ok(());
        };
        let from_omnislice = query
            .card
            .is_some_and(|card| eng.s.card(card).kind == CardKind::Omnislice);
        if query.target == me.owner && (query.props.is_powered_attack() || from_omnislice) {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            // `Unpowered | SkipHurtAnim`; the animation flag is cosmetic.
            eng.damage(
                &[dealer],
                amount,
                ValueProps::UNPOWERED,
                Some(me.owner),
                None,
            )?;
        }
        Ok(())
    }
}
