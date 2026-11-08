//! Test-only relic for `ModifyUnknownMapPointRoomTypes`, shaped like
//! `GoldenCompass.ModifyUnknownMapPointRoomTypes` (`Models.Relics/GoldenCompass.cs:48-55`) but
//! with shops: every `?` becomes a shop.

#![cfg(test)]

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RoomType, RunState};

/// Test-only: `ModifyUnknownMapPointRoomTypes`.
pub struct TestUnknownShops;

impl RelicModel for TestUnknownShops {
    fn name(&self) -> &'static str {
        "Test Unknown Shops"
    }

    fn modify_unknown_map_point_room_types(
        &self,
        _s: &RunState,
        _me: usize,
        types: &mut Vec<RoomType>,
    ) {
        *types = vec![RoomType::Shop];
    }
}

#[sts2_macros::hooks]
impl Hooks for TestUnknownShops {
    type Me = RelicId;
}
