//! `WhiteBeastStatue` (`Models.Relics/WhiteBeastStatue.cs`): every combat room's rewards include a
//! potion (`ShouldForcePotionReward`). `IsAllowed`: before the Act 3 treasure.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{RoomType, RunState};

pub struct WhiteBeastStatue;

impl RelicModel for WhiteBeastStatue {
    /// `IsAllowed` (`WhiteBeastStatue.cs:12-15`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "White Beast Statue"
    }

    fn should_force_potion_reward(&self, _s: &RunState, _me: usize, room: RoomType) -> bool {
        room.is_combat_room()
    }
}

#[sts2_macros::hooks]
impl Hooks for WhiteBeastStatue {
    type Me = RelicId;
}
