//! Test-only relic for `ModifyGeneratedMapLate`, shaped like `SpoilsMap.ModifyGeneratedMapLate`
//! (`Models.Cards/SpoilsMap.cs:63-80`): it notes the final map's first treasure point.

#![cfg(test)]

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::RunState;
use crate::run::map::{ActMap, MapCoord, MapError, MapPointType};

/// Test-only: `ModifyGeneratedMapLate`.
pub struct TestMapMarker;

/// `TestMapMarker`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// The act and first treasure point of the last map generated.
    pub marked: Option<(usize, MapCoord)>,
}

impl RelicModel for TestMapMarker {
    fn name(&self) -> &'static str {
        "Test Map Marker"
    }

    fn init_data(&self) -> RelicData {
        RelicData::TestMapMarker(Data::default())
    }

    fn modify_generated_map_late(
        &self,
        s: &mut RunState,
        me: usize,
        map: &mut ActMap,
        act_index: usize,
    ) -> Result<(), MapError> {
        let RelicData::TestMapMarker(data) = &mut s.relics[me].data else {
            unreachable!("a TestMapMarker has TestMapMarker data");
        };
        data.marked = map
            .points()
            .iter()
            .find(|p| p.kind == MapPointType::Treasure)
            .map(|p| (act_index, p.coord));
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for TestMapMarker {
    type Me = RelicId;
}
