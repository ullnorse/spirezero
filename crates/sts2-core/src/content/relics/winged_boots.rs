//! `WingedBoots` (`Models.Relics/WingedBoots.cs`): 3 times, the player may travel to any point
//! of the next row (`ShouldAllowFreeTravel`); entering a room that is not a child of the
//! previous point uses one (`AfterRoomEntered`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::RunState;

/// `_roomCount`.
const ROOMS: i32 = 3;

pub struct WingedBoots;

/// `WingedBoots`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `TimesUsed`.
    pub times_used: i32,
}

fn data_mut(s: &mut RunState, me: usize) -> &mut Data {
    match &mut s.relics[me].data {
        RelicData::WingedBoots(data) => data,
        _ => unreachable!("a WingedBoots has WingedBoots data"),
    }
}

/// `ShouldAllowFreeTravel`: `!IsUsedUp`.
#[must_use]
pub fn allows_free_travel(s: &RunState, me: usize) -> bool {
    matches!(s.relics[me].data, RelicData::WingedBoots(d) if d.times_used < ROOMS)
}

/// `AfterRoomEntered` for a room entered from the map, the only room on the stack:
/// `TimesUsed++` unless used up or the room's point is a child of the previous visited point.
pub fn after_room_entered(s: &mut RunState, me: usize) {
    if !allows_free_travel(s, me) || s.visited.len() <= 1 {
        return;
    }
    let previous = s.visited[s.visited.len() - 2];
    let current = s.visited[s.visited.len() - 1];
    let Some(point) = s.map.point(previous) else {
        return;
    };
    if !point.children.contains(&current) {
        data_mut(s, me).times_used += 1;
    }
}

impl RelicModel for WingedBoots {
    fn name(&self) -> &'static str {
        "Winged Boots"
    }

    fn init_data(&self) -> RelicData {
        RelicData::WingedBoots(Data::default())
    }

    /// `IsUsedUp => TimesUsed >= 3`.
    fn is_used_up(&self, data: &RelicData) -> bool {
        matches!(data, RelicData::WingedBoots(d) if d.times_used >= ROOMS)
    }
}

#[sts2_macros::hooks]
impl Hooks for WingedBoots {
    type Me = RelicId;
}
