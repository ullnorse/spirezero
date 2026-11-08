//! `Orichalcum` (`Models.Relics/Orichalcum.cs`): 6 Block at the end of the owner's turn if it has
//! none. The check runs in the very early hook, before `PlatingPower` gives its Block.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `BlockVar(6m, ValueProp.Unpowered)`.
const BLOCK: i32 = 6;

pub struct Orichalcum;

/// `Orichalcum`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `ShouldTrigger`.
    pub should_trigger: bool,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Orichalcum(data) = &mut s.relic_mut(me).data else {
        unreachable!("an Orichalcum has Orichalcum data");
    };
    data
}

impl RelicModel for Orichalcum {
    fn name(&self) -> &'static str {
        "Orichalcum"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Orichalcum(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Orichalcum {
    type Me = RelicId;

    fn before_side_turn_end_very_early(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        let owner = eng.s.player.creature;
        if participants.contains(&owner) && eng.s.creature(owner).block <= 0 {
            data_mut(eng.s, me).should_trigger = true;
        }
        Ok(())
    }

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        let data = data_mut(eng.s, me);
        if data.should_trigger {
            data.should_trigger = false;
            let owner = eng.s.player.creature;
            eng.gain_block(owner, Decimal::from(BLOCK), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            data_mut(eng.s, me).should_trigger = false;
        }
        Ok(())
    }
}
