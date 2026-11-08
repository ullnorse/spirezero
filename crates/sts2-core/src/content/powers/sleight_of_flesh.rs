//! `SleightOfFleshPower` (`Models.Powers/SleightOfFleshPower.cs`): when the owner applies a
//! non-temporary debuff to an enemy (the change's type is `Debuff`), deal `Amount` unpowered
//! damage to that enemy with the owner as dealer.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SleightOfFlesh;

impl PowerModel for SleightOfFlesh {
    fn name(&self) -> &'static str {
        "Sleight of Flesh"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SleightOfFlesh {
    type Me = PowerRef;

    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        // The changed instance may already be off its owner (an earlier listener killed it).
        let kind = eng.s.power(power.owner, power.id).kind;
        if amount == Decimal::ZERO
            || kind.type_for_amount(amount) != PowerType::Debuff
            || eng.s.creature(power.owner).side != Side::Enemy
            || applier != Some(me.owner)
            || kind.is_temporary()
        {
            return Ok(());
        }
        let damage = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.damage(
            &[power.owner],
            damage,
            ValueProps::UNPOWERED,
            Some(me.owner),
            None,
        )?;
        Ok(())
    }
}
