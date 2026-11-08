//! `FakeOrichalcum` (`Models.Relics/FakeOrichalcum.cs`): `FakeMerchant`'s `Orichalcum`, 3 Block at
//! the end of the owner's turn if it has none. Its field is `Orichalcum`'s `ShouldTrigger`, kept
//! in [`RelicData::Orichalcum`].

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::relics::{RelicData, RelicModel, orichalcum};
use crate::decimal::Decimal;

/// `BlockVar(3m, ValueProp.Unpowered)`.
const BLOCK: i32 = 3;

pub struct FakeOrichalcum;

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut orichalcum::Data {
    let RelicData::Orichalcum(data) = &mut s.relic_mut(me).data else {
        unreachable!("a FakeOrichalcum has Orichalcum data");
    };
    data
}

impl RelicModel for FakeOrichalcum {
    fn name(&self) -> &'static str {
        "Orichalcum???"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Orichalcum(orichalcum::Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeOrichalcum {
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
