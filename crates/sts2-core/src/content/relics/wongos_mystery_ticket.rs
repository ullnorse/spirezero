//! `WongosMysteryTicket` (`Models.Relics/WongosMysteryTicket.cs`): after 5 finished combats, the
//! next combat room's rewards gain 3 relic rewards, once. Event rarity (`WelcomeToWongos`).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{PendingReward, Reward, RoomType, RunState};

/// `combatsToActivate`.
const COMBATS: i32 = 5;
/// `RepeatVar(3)` (`relicCount`).
const RELICS: usize = 3;

pub struct WongosMysteryTicket;

/// `WongosMysteryTicket`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `[SavedProperty] CombatsFinished`.
    pub combats_finished: i32,
    /// `[SavedProperty] GaveRelic`.
    pub gave_relic: bool,
}

fn data(relic: &RelicData) -> &Data {
    let RelicData::WongosMysteryTicket(data) = relic else {
        unreachable!("a WongosMysteryTicket has WongosMysteryTicket data");
    };
    data
}

fn data_mut(relic: &mut RelicData) -> &mut Data {
    let RelicData::WongosMysteryTicket(data) = relic else {
        unreachable!("a WongosMysteryTicket has WongosMysteryTicket data");
    };
    data
}

impl RelicModel for WongosMysteryTicket {
    fn name(&self) -> &'static str {
        "Wongo's Mystery Ticket"
    }

    fn init_data(&self) -> RelicData {
        RelicData::WongosMysteryTicket(Data::default())
    }

    /// `IsUsedUp => GaveRelic`.
    fn is_used_up(&self, relic: &RelicData) -> bool {
        data(relic).gave_relic
    }

    /// `TryModifyRewards`: a combat room's rewards, not yet given, with no combats remaining.
    fn try_modify_rewards(
        &self,
        s: &mut RunState,
        me: usize,
        room: Option<RoomType>,
        _rewards: &[Reward],
        added: &mut Vec<PendingReward>,
    ) -> bool {
        if !room.is_some_and(RoomType::is_combat_room) {
            return false;
        }
        let data = data(&s.relics[me].data);
        if data.gave_relic || COMBATS - data.combats_finished > 0 {
            return false;
        }
        added.extend(std::iter::repeat_n(PendingReward::Relic, RELICS));
        true
    }

    /// `AfterModifyingRewards`: `GaveRelic = true`.
    fn after_modifying_rewards(&self, s: &mut RunState, me: usize) {
        data_mut(&mut s.relics[me].data).gave_relic = true;
    }
}

#[sts2_macros::hooks]
impl Hooks for WongosMysteryTicket {
    type Me = RelicId;

    /// `AfterCombatEnd`: `CombatsFinished++` (the `RemainingCombats` var is text only).
    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(&mut eng.s.relic_mut(me).data).combats_finished += 1;
        Ok(())
    }
}
