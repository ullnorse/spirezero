//! `GoldenCompass` (`Models.Relics/GoldenCompass.cs`): Ancient (Tezcatara); `AfterObtained`
//! records the current act (`GoldenPathAct`) and regenerates the map
//! (`RunManager.Instance.GenerateMap()`); that act's map is the `GoldenPathActMap`
//! (`ModifyGeneratedMap`) and its `?` rooms are always events
//! (`ModifyUnknownMapPointRoomTypes`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::map::{ActMap, MapError};
use crate::run::{RoomType, RunCtx, RunError, RunState};

pub struct GoldenCompass;

/// `GoldenCompass`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `GoldenPathAct`: the act index whose map is the golden path, -1 for none.
    pub golden_path_act: i32,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            golden_path_act: -1,
        }
    }
}

fn golden_path_act(s: &RunState, me: usize) -> i32 {
    let RelicData::GoldenCompass(data) = s.relics[me].data else {
        unreachable!("a GoldenCompass has GoldenCompass data");
    };
    data.golden_path_act
}

/// `CurrentActIndex` (or a map's `actIndex`) as the field's `int`.
fn act(index: usize) -> i32 {
    i32::try_from(index).expect("an act index fits i32")
}

impl RelicModel for GoldenCompass {
    fn name(&self) -> &'static str {
        "Golden Compass"
    }

    fn init_data(&self) -> RelicData {
        RelicData::GoldenCompass(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let current = act(s.act_index);
        s.relics[me].data = RelicData::GoldenCompass(Data {
            golden_path_act: current,
        });
        ctx.regenerate_map()
    }

    fn modify_generated_map(
        &self,
        s: &mut RunState,
        me: usize,
        map: &mut ActMap,
        act_index: usize,
    ) -> Result<(), MapError> {
        if golden_path_act(s, me) == act(act_index) {
            *map = ActMap::golden_path();
        }
        Ok(())
    }

    fn modify_unknown_map_point_room_types(
        &self,
        s: &RunState,
        me: usize,
        types: &mut Vec<RoomType>,
    ) {
        if golden_path_act(s, me) == act(s.act_index) {
            *types = vec![RoomType::Event];
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for GoldenCompass {
    type Me = RelicId;
}
