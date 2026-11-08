//! `BattlewornDummyTimeLimitPower` (`Models.Powers/BattlewornDummyTimeLimitPower.cs`): counts the
//! `BattleFriend`'s own turns down; when its last turn ends the dummy escapes and the encounter
//! remembers that the player ran out of time.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::encounters::{Encounter, EncounterData};
use crate::content::powers::{PowerModel, PowerType};

pub struct BattlewornDummyTimeLimit;

impl PowerModel for BattlewornDummyTimeLimit {
    fn name(&self) -> &'static str {
        "Time Limit"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for BattlewornDummyTimeLimit {
    type Me = PowerRef;

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        if eng.s.power(me.owner, me.id).amount > 1 {
            return eng.decrement_power(me);
        }
        if matches!(
            eng.s.encounter,
            Encounter::BattlewornDummyEventV1Encounter
                | Encounter::BattlewornDummyEventV2Encounter
                | Encounter::BattlewornDummyEventV3Encounter
        ) {
            eng.s.encounter_data = EncounterData::BattlewornDummy {
                ran_out_of_time: true,
            };
        }
        eng.escape(me.owner);
        Ok(())
    }
}
