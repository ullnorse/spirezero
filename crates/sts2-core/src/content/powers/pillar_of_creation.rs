//! `PillarOfCreationPower` (`Models.Powers/PillarOfCreationPower.cs`): a counter buff; whenever
//! the owner creates a card for the combat, it gains `Amount` Unpowered block.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::{PowerModel, PowerType};

pub struct PillarOfCreation;

impl PowerModel for PillarOfCreation {
    fn name(&self) -> &'static str {
        "Pillar of Creation"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for PillarOfCreation {
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
            eng.gain_block(me.owner, amount.into(), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
