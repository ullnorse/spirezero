//! `ArsenalPower` (`Models.Powers/ArsenalPower.cs`): a counter buff; whenever the owner creates
//! a card for the combat, it gains `Amount` Strength.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CreatureId, Engine, Flow};
use crate::content::powers::{PowerKind, PowerModel, PowerType};

pub struct Arsenal;

impl PowerModel for Arsenal {
    fn name(&self) -> &'static str {
        "Arsenal"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Arsenal {
    type Me = PowerRef;

    fn after_card_generated_for_combat(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        creator: Option<CreatureId>,
    ) -> Flow {
        if creator == Some(me.owner) {
            // Flash: visual only.
            let amount = eng.s.power(me.owner, me.id).amount;
            eng.apply_power(me.owner, PowerKind::Strength, amount.into(), Some(me.owner))?;
        }
        Ok(())
    }
}
