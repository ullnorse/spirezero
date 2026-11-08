//! `WhiteStar` (`Models.Relics/WhiteStar.cs`): elite rooms offer a second card reward with boss
//! odds (`CardCreationOptions.ForRoom(Boss)`, 3 cards).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarityOddsType;
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::{CardCreation, PendingReward, Reward, RoomType, RunState};

/// The cards of the extra reward.
const CARDS: usize = 3;

pub struct WhiteStar;

impl RelicModel for WhiteStar {
    /// `IsAllowed` (`WhiteStar.cs:14-17`).
    fn is_allowed(&self, s: &RunState) -> bool {
        is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "White Star"
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
        added.push(PendingReward::Card(
            CardCreation::for_room(CardRarityOddsType::BossEncounter),
            CARDS,
        ));
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for WhiteStar {
    type Me = RelicId;
}
