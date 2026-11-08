//! `Vintage` (`Models.Modifiers/Vintage.cs`): in a monster room (not an elite or boss) every card
//! reward becomes a relic reward (`TryModifyRewardsLate` replaces it in place with
//! `RelicReward(player)`, which is populated after the set's own rewards: a
//! `PullNextRelicFromFront`).

use crate::run::rewards::{PendingReward, Reward, pull_relic_from_front};
use crate::run::{RoomType, RunState};

/// `TryModifyRewardsLate`. The rewards the room made are populated, so their relic is pulled now
/// (they come before the added ones in the set's order); an added card reward becomes a pending
/// relic.
pub(super) fn relics_for_cards(
    s: &mut RunState,
    room: Option<RoomType>,
    rewards: &mut [Reward],
    added: &mut [PendingReward],
) {
    if room != Some(RoomType::Monster) {
        return;
    }
    for reward in rewards {
        if matches!(reward, Reward::Card(_)) {
            *reward = Reward::Relic(pull_relic_from_front(s));
        }
    }
    for pending in added {
        if matches!(
            pending,
            PendingReward::Card(..) | PendingReward::RerollableCard(..)
        ) {
            *pending = PendingReward::Relic;
        }
    }
}
