//! `Driftwood` (`Models.Relics/Driftwood.cs`): every card reward can be rerolled once
//! (`TryModifyRewardsLate` sets `CardReward.CanReroll`; [`crate::run::RunAction::RerollCards`]).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{PendingReward, Reward, RoomType, RunState};

pub struct Driftwood;

impl RelicModel for Driftwood {
    fn name(&self) -> &'static str {
        "Driftwood"
    }

    /// Every `CardReward` of the set, those other listeners added included, in any room.
    fn try_modify_rewards_late(
        &self,
        _s: &mut RunState,
        _me: usize,
        _room: Option<RoomType>,
        rewards: &mut [Reward],
        added: &mut [PendingReward],
    ) -> bool {
        for reward in rewards {
            if let Reward::Card(cards) = reward {
                cards.can_reroll = true;
            }
        }
        for reward in added {
            if let PendingReward::Card(options, count) = *reward {
                *reward = PendingReward::RerollableCard(options, count);
            }
        }
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Driftwood {
    type Me = RelicId;
}
