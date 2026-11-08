//! `Planisphere` (`Models.Relics/Planisphere.cs`): heals 5 when a room entered from a `?` point
//! begins (`AfterRoomEntered`, `CurrentRoomCount <= 1`: an event's own fight does not count).
//! A `?` that became a fight is handled by [`RelicModel::after_map_combat_room_entered`]: the
//! heal comes before the fight starts, as `AfterRoomEntered` is its first hook.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::map::MapPointType;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RoomType, RunState};

/// `HealVar(5m)`.
const HEAL: i32 = 5;

pub struct Planisphere;

fn heal_on_unknown(s: &mut RunState, point_type: Option<MapPointType>) {
    if s.hp > 0 && point_type == Some(MapPointType::Unknown) {
        s.heal(Decimal::from(HEAL));
    }
}

impl RelicModel for Planisphere {
    /// `IsAllowed` (`Planisphere.cs:18-21`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Planisphere"
    }

    fn after_room_entered_out_of_combat(&self, s: &mut RunState, _me: usize, _room: RoomType) {
        let point_type = s.history.last().map(|h| h.point_type);
        heal_on_unknown(s, point_type);
    }

    fn after_map_combat_room_entered(&self, s: &mut RunState, _me: usize, point: MapPointType) {
        heal_on_unknown(s, Some(point));
    }
}

#[sts2_macros::hooks]
impl Hooks for Planisphere {
    type Me = RelicId;
}
