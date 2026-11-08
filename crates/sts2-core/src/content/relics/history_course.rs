//! `HistoryCourse` (`Models.Relics/HistoryCourse.cs`): Event (`WarHistorianRepy`'s cage); when
//! the auto-pre-play phase begins, from turn 2, a dupe of the last Attack (not a dupe) the owner
//! finished playing last turn is auto-played (`CardCmd.AutoPlay(CreateDupe)`, no target: an
//! `AnyEnemy` attack picks one on `CombatTargets`).

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;

pub struct HistoryCourse;

impl RelicModel for HistoryCourse {
    /// Auto-plays a copy of last turn's last card; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "History Course"
    }
}

#[sts2_macros::hooks]
impl Hooks for HistoryCourse {
    type Me = RelicId;

    /// `CardPlaysFinished.LastOrDefault` of the owner's non-dupe Attacks with
    /// `HappenedLastPlayerTurn` (the entry's player turn number is `TurnNumber - 1`).
    fn after_auto_pre_play_phase_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        let turn = eng.s.player.turn_number;
        if turn == 1 {
            return Ok(());
        }
        let Some(card) = eng.s.last_attack_played_in_turn(turn - 1) else {
            return Ok(());
        };
        let dupe = eng.create_dupe(card);
        eng.auto_play(dupe, None)
    }
}
