//! `DoubleDamagePower` (`Models.Powers/DoubleDamagePower.cs`): a buff (`Counter`); the owner's (and
//! its pets') powered card attacks deal double damage; loses a stack when its side's turn ends.

use crate::combat::hooks::{DamageQuery, Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct DoubleDamage;

impl PowerModel for DoubleDamage {
    fn name(&self) -> &'static str {
        "Double Damage"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for DoubleDamage {
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
        if !from_owner || !query.props.is_powered_attack() || query.card.is_none() {
            return Decimal::ONE;
        }
        Decimal::from(2)
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
