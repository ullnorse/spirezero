//! `PrayerWheel` (`Models.Relics/PrayerWheel.cs`): monster rooms offer a second card reward
//! (`CardCreationOptions.ForRoom(Monster)`, 3 cards).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarityOddsType;
use crate::run::{CardCreation, PendingReward, Reward, RoomType, RunState};

/// The cards of the extra reward.
const CARDS: usize = 3;

pub struct PrayerWheel;

impl RelicModel for PrayerWheel {
    fn name(&self) -> &'static str {
        "Prayer Wheel"
    }

    fn try_modify_rewards(
        &self,
        _s: &mut RunState,
        _me: usize,
        room: Option<RoomType>,
        _rewards: &[Reward],
        added: &mut Vec<PendingReward>,
    ) -> bool {
        if room != Some(RoomType::Monster) {
            return false;
        }
        added.push(PendingReward::Card(
            CardCreation::for_room(CardRarityOddsType::RegularEncounter),
            CARDS,
        ));
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for PrayerWheel {
    type Me = RelicId;
}
