//! The `Draft` modifier.
//!
//! `Draft` (`Models.Modifiers/Draft.cs`): the deck starts empty; Neow's option offers 10 card
//! rewards of 3 cards one after another (character pool, `RegularEncounter` base odds, source
//! `Other`, no upgrade roll), none skippable; then `PandorasBox` leaves the relic bags.

use crate::run::RunState;
use crate::run::rewards::{CardCreation, CardPool, CardReward, Reward};

/// The rewards offered.
pub const PICKS: u8 = 10;

/// The cards in each reward.
const OFFERED: usize = 3;

/// One `CardReward` of the draft (`new CardReward(options, 3, player)`, `Populate`).
pub(super) fn reward(s: &mut RunState) -> Reward {
    let options = CardCreation::non_combat_default(CardPool::Character);
    Reward::Card(CardReward::generate(s, OFFERED, options))
}
