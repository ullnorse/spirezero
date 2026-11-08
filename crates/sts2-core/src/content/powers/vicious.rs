//! `ViciousPower` (`Models.Powers/ViciousPower.cs`): the owner draws `Amount` cards whenever it
//! actually lands Vulnerable (a positive change applied by the owner; Artifact-blocked applications
//! never reach `AfterPowerAmountChanged`).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::{PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Vicious;

impl PowerModel for Vicious {
    fn name(&self) -> &'static str {
        "Vicious"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Vicious {
    type Me = PowerRef;

    fn after_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerRef,
        amount: Decimal,
        applier: Option<CreatureId>,
    ) -> Flow {
        if amount <= Decimal::ZERO
            || applier != Some(me.owner)
            || eng.s.power(power.owner, power.id).kind != PowerKind::Vulnerable
        {
            return Ok(());
        }
        let cards = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.draw(cards, false)?;
        Ok(())
    }
}
