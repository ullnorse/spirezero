//! `AmethystAubergine` (`Models.Relics/AmethystAubergine.cs`): combat rooms (but the last act's
//! boss) offer 15 more gold.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{PendingReward, Reward, RoomType, RunState};

/// `GoldVar(15)`.
const GOLD: i32 = 15;

pub struct AmethystAubergine;

impl RelicModel for AmethystAubergine {
    /// `IsAllowed` (`AmethystAubergine.cs:20-23`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Amethyst Aubergine"
    }

    fn try_modify_rewards(
        &self,
        s: &mut RunState,
        _me: usize,
        room: Option<RoomType>,
        _rewards: &[Reward],
        added: &mut Vec<PendingReward>,
    ) -> bool {
        let Some(room) = room.filter(|r| r.is_combat_room()) else {
            return false;
        };
        if room == RoomType::Boss && s.act_index + 1 >= s.acts.len() {
            return false;
        }
        // `new GoldReward(amount, player)` is populated at once.
        added.push(PendingReward::Ready(Reward::Gold(GOLD)));
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for AmethystAubergine {
    type Me = RelicId;
}
