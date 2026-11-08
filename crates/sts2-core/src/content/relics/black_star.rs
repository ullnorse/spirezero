//! `BlackStar` (`Models.Relics/BlackStar.cs`): an elite fight's rewards get one more relic
//! (`TryModifyRewards` adds a `RelicReward(player)`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{PendingReward, Reward, RoomType, RunState};

pub struct BlackStar;

impl RelicModel for BlackStar {
    fn name(&self) -> &'static str {
        "Black Star"
    }

    fn try_modify_rewards(
        &self,
        _s: &mut RunState,
        _me: usize,
        room: Option<RoomType>,
        _rewards: &[Reward],
        added: &mut Vec<PendingReward>,
    ) -> bool {
        if room != Some(RoomType::Elite) {
            return false;
        }
        added.push(PendingReward::Relic);
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for BlackStar {
    type Me = RelicId;
}
