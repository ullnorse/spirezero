//! `Midas` (`Models.Modifiers/Midas.cs`): gold rewards are doubled (`TryModifyRewardsLate`
//! replaces each `GoldReward` by one of twice the amount), and the Smith rest-site option is
//! removed (`TryModifyRestSiteOptions`).

use crate::run::rewards::{PendingReward, Reward};
use crate::run::{RestOption, RestOptionKind};

/// `TryModifyRewardsLate`: every populated gold reward, in the set and among the ones listeners
/// added, pays twice.
pub(super) fn double_gold(rewards: &mut [Reward], added: &mut [PendingReward]) {
    for reward in rewards {
        if let Reward::Gold(amount) = reward {
            *amount *= 2;
        }
    }
    for pending in added {
        if let PendingReward::Ready(Reward::Gold(amount)) = pending {
            *amount *= 2;
        }
    }
}

/// `TryModifyRestSiteOptions`: no Smith.
pub(super) fn remove_smith(options: &mut Vec<RestOption>) {
    options.retain(|o| o.kind != RestOptionKind::Smith);
}
