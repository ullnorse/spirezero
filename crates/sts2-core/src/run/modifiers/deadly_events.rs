//! `DeadlyEvents` (`Models.Modifiers/DeadlyEvents.cs`): a `?` can be an elite from the start (its
//! odds and base odds are 0.1 instead of -1), a treasure's odds grow twice as fast
//! (`ModifyOddsIncreaseForUnrolledRoomType`), and `JuzuBracelet` (which stops `?` fights) is
//! taken out of the relic bags.

use crate::run::{RoomType, RunState};

/// `EliteOdds` / `SetBaseOdds(Elite)`.
const ELITE_ODDS: f32 = 0.1;

/// `AfterRunCreated`.
pub(super) fn after_run_created(s: &mut RunState) {
    s.player_relic_bag.remove("JUZU_BRACELET");
    s.shared_relic_bag.remove("JUZU_BRACELET");
    s.unknown_odds.set_base(RoomType::Elite, ELITE_ODDS);
    s.unknown_odds.reset_to_base();
    s.unknown_odds.double_treasure_increase();
}
