//! `DreamCatcher` (`Models.Relics/DreamCatcher.cs`): a rest-site heal also offers a card reward of
//! 3 cards (`CardCreationOptions.ForRoom(Monster)`); `ModifyExtraRestSiteHealText` only changes
//! the display.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::odds::CardRarityOddsType;
use crate::run::{CardCreation, PendingReward, RunState};

/// The cards of the extra reward.
const CARDS: usize = 3;

pub struct DreamCatcher;

impl RelicModel for DreamCatcher {
    fn name(&self) -> &'static str {
        "Dream Catcher"
    }

    fn try_modify_rest_site_heal_rewards(
        &self,
        _s: &RunState,
        _me: usize,
        rewards: &mut Vec<PendingReward>,
        _is_mimicked: bool,
    ) -> bool {
        rewards.push(PendingReward::Card(
            CardCreation::for_room(CardRarityOddsType::RegularEncounter),
            CARDS,
        ));
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for DreamCatcher {
    type Me = RelicId;
}
