//! `JuzuBracelet` (`Models.Relics/JuzuBracelet.cs`): `?` rooms no longer roll monsters.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RoomType, RunState};

pub struct JuzuBracelet;

impl RelicModel for JuzuBracelet {
    /// `IsAllowed` (`JuzuBracelet.cs:12-15`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Juzu Bracelet"
    }

    fn modify_unknown_map_point_room_types(
        &self,
        _s: &RunState,
        _me: usize,
        types: &mut Vec<RoomType>,
    ) {
        types.retain(|&t| t != RoomType::Monster);
    }
}

#[sts2_macros::hooks]
impl Hooks for JuzuBracelet {
    type Me = RelicId;
}
