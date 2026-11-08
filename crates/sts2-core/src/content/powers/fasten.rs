//! `FastenPower` (`Models.Powers/FastenPower.cs`): the owner's powered Block from cards tagged
//! `Defend` is `Amount` higher.

use crate::combat::CombatState;
use crate::combat::hooks::{BlockQuery, Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Fasten;

impl PowerModel for Fasten {
    fn name(&self) -> &'static str {
        "Fasten"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Fasten {
    type Me = PowerRef;

    fn modify_block_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        if query.target != me.owner || !query.props.is_powered_card_or_monster_move_block() {
            return Decimal::ZERO;
        }
        if query
            .card
            .is_some_and(|card| !s.card(card).kind.is_defend())
        {
            return Decimal::ZERO;
        }
        Decimal::from(s.power(me.owner, me.id).amount)
    }
}
