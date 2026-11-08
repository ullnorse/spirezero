//! `DexterityPower` (`Models.Powers/DexterityPower.cs`): `+Amount` to the owner's powered card
//! and monster-move Block.

use crate::combat::CombatState;
use crate::combat::hooks::{BlockQuery, Hooks, PowerRef};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Dexterity;

impl PowerModel for Dexterity {
    fn name(&self) -> &'static str {
        "Dexterity"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn allows_negative(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Dexterity {
    type Me = PowerRef;

    fn modify_block_additive(
        &self,
        s: &CombatState,
        me: PowerRef,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        // `cardSource.Owner.Creature != Owner` is always false for the only player's cards, so a
        // card source falls through to the type test; with none the target must be the owner.
        if query.card.is_none() && query.target != me.owner {
            return Decimal::ZERO;
        }
        if query.card.is_some() && s.player.creature != me.owner {
            return Decimal::ZERO;
        }
        if !query.props.is_powered_card_or_monster_move_block() {
            return Decimal::ZERO;
        }
        Decimal::from(s.power(me.owner, me.id).amount)
    }
}
