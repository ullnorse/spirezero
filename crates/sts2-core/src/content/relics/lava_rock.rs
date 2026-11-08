//! `LavaRock` (`Models.Relics/LavaRock.cs`): the Act 1 boss offers 2 more relic rewards, once.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{PendingReward, Reward, RoomType, RunState};

/// `DynamicVar("Relics", 2m)`.
const RELICS: usize = 2;

pub struct LavaRock;

/// `LavaRock`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `HasTriggered`.
    pub has_triggered: bool,
}

impl RelicModel for LavaRock {
    fn name(&self) -> &'static str {
        "Lava Rock"
    }

    fn init_data(&self) -> RelicData {
        RelicData::LavaRock(Data::default())
    }

    fn try_modify_rewards(
        &self,
        s: &mut RunState,
        me: usize,
        room: Option<RoomType>,
        _rewards: &[Reward],
        added: &mut Vec<PendingReward>,
    ) -> bool {
        if room != Some(RoomType::Boss) || s.act_index != 0 {
            return false;
        }
        let RelicData::LavaRock(data) = &mut s.relics[me].data else {
            unreachable!("a LavaRock has LavaRock data");
        };
        if data.has_triggered {
            return false;
        }
        added.extend(std::iter::repeat_n(PendingReward::Relic, RELICS));
        data.has_triggered = true;
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for LavaRock {
    type Me = RelicId;
}
