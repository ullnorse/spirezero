//! `VenerableTeaSet` (`Models.Relics/VenerableTeaSet.cs`): entering a rest site arms it; the next
//! fight starts with 2 more energy on its first turn (`AfterEnergyReset`).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::run::{RoomType, RunState};

/// `EnergyVar(2)`.
const ENERGY: i32 = 2;

pub struct VenerableTeaSet;

/// `VenerableTeaSet`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `GainEnergyInNextCombat`.
    pub gain_energy_in_next_combat: bool,
}

impl RelicModel for VenerableTeaSet {
    fn name(&self) -> &'static str {
        "Venerable Tea Set"
    }

    fn init_data(&self) -> RelicData {
        RelicData::VenerableTeaSet(Data::default())
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, me: usize, room: RoomType) {
        if room == RoomType::RestSite {
            s.relics[me].data = RelicData::VenerableTeaSet(Data {
                gain_energy_in_next_combat: true,
            });
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for VenerableTeaSet {
    type Me = RelicId;

    fn after_energy_reset(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::VenerableTeaSet(data) = eng.s.relic(me).data else {
            unreachable!("a VenerableTeaSet has VenerableTeaSet data");
        };
        if data.gain_energy_in_next_combat {
            eng.gain_energy(Decimal::from(ENERGY))?;
            eng.s.relic_mut(me).data = RelicData::VenerableTeaSet(Data::default());
        }
        Ok(())
    }
}
