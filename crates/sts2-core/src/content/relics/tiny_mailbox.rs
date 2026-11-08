//! `TinyMailbox` (`Models.Relics/TinyMailbox.cs`): a rest-site heal also offers 2 potion rewards
//! (`TryModifyRestSiteHealRewards`); `ModifyExtraRestSiteHealText` only changes the display.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{PendingReward, RunState};

/// The two `new PotionReward(player)`.
const POTIONS: usize = 2;

pub struct TinyMailbox;

impl RelicModel for TinyMailbox {
    fn name(&self) -> &'static str {
        "Tiny Mailbox"
    }

    fn try_modify_rest_site_heal_rewards(
        &self,
        _s: &RunState,
        _me: usize,
        rewards: &mut Vec<PendingReward>,
        _is_mimicked: bool,
    ) -> bool {
        rewards.extend(std::iter::repeat_n(PendingReward::Potion, POTIONS));
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for TinyMailbox {
    type Me = RelicId;
}
