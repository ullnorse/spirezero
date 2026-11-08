//! `FakeVenerableTeaSet` (`Models.Relics/FakeVenerableTeaSet.cs`): `FakeMerchant`'s
//! `VenerableTeaSet`; entering a rest site arms it, and the next fight starts with 1 more energy on
//! its first turn (`AfterEnergyReset`). Its field is `VenerableTeaSet`'s `GainEnergyInNextCombat`,
//! kept in [`RelicData::VenerableTeaSet`].

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel, venerable_tea_set};
use crate::decimal::Decimal;
use crate::run::{RoomType, RunState};

/// `EnergyVar(1)`.
const ENERGY: i32 = 1;

pub struct FakeVenerableTeaSet;

impl RelicModel for FakeVenerableTeaSet {
    fn name(&self) -> &'static str {
        "Venerable Tea Set???"
    }

    fn init_data(&self) -> RelicData {
        RelicData::VenerableTeaSet(venerable_tea_set::Data::default())
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, me: usize, room: RoomType) {
        if room == RoomType::RestSite {
            s.relics[me].data = RelicData::VenerableTeaSet(venerable_tea_set::Data {
                gain_energy_in_next_combat: true,
            });
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for FakeVenerableTeaSet {
    type Me = RelicId;

    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::VenerableTeaSet(data) = eng.s.relic(me).data else {
            unreachable!("a FakeVenerableTeaSet has VenerableTeaSet data");
        };
        if data.gain_energy_in_next_combat {
            eng.gain_energy(Decimal::from(ENERGY))?;
            eng.s.relic_mut(me).data =
                RelicData::VenerableTeaSet(venerable_tea_set::Data::default());
        }
        Ok(())
    }
}
