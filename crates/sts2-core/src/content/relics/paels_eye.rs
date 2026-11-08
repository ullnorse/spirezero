//! `PaelsEye` (`Models.Relics/PaelsEye.cs`): Ancient (Pael); once per combat, a player turn in
//! which the owner played no card (auto-plays do not count) exhausts its hand at
//! `BeforeSideTurnEndEarly` and is followed by an extra turn (`ShouldTakeExtraTurn`).
//! `WhisperingEarring` auto-plays the first turn, so with it turn 1 counts as played. `Status` is
//! display only.

use crate::combat::history::{HistoryKind, HistoryTag};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{RunCtx, RunError};

pub struct PaelsEye;

/// `PaelsEye`'s fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_usedThisCombat`.
    pub used_this_combat: bool,
    /// `_wasOwnerPartOfLastPlayerTurn`.
    pub was_owner_part_of_last_player_turn: bool,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            used_this_combat: false,
            was_owner_part_of_last_player_turn: true,
        }
    }
}

fn data(s: &CombatState, me: RelicId) -> Data {
    let RelicData::PaelsEye(data) = s.relic(me).data else {
        unreachable!("a PaelsEye has PaelsEye data");
    };
    data
}

fn data_mut<'a>(eng: &'a mut Engine<'_>, me: RelicId) -> &'a mut Data {
    let RelicData::PaelsEye(data) = &mut eng.s.relic_mut(me).data else {
        unreachable!("a PaelsEye has PaelsEye data");
    };
    data
}

/// `AnyCardsPlayedThisTurn` (`PaelsEye.cs:150-158`).
fn any_cards_played_this_turn(s: &CombatState) -> bool {
    if s.player.turn_number == 1
        && s.player
            .relics
            .iter()
            .any(|r| r.kind.id() == "WHISPERING_EARRING")
    {
        return true;
    }
    let player = s.player.creature;
    s.history_this_turn(HistoryTag::CardPlayFinished)
        .iter()
        .any(|e| {
            e.actor == player
                && matches!(&e.kind, HistoryKind::CardPlayFinished { play, .. } if !play.is_auto_play)
        })
}

/// `!UsedThisCombat && !AnyCardsPlayedThisTurn() && WasOwnerPartOfLastPlayerTurn`.
fn triggers(s: &CombatState, me: RelicId) -> bool {
    let d = data(s, me);
    !d.used_this_combat && !any_cards_played_this_turn(s) && d.was_owner_part_of_last_player_turn
}

impl RelicModel for PaelsEye {
    fn name(&self) -> &'static str {
        "Pael's Eye"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PaelsEye(Data::default())
    }

    /// `CombatManager.Instance.IsPartOfPlayerTurn(Owner)`: false out of combat (no turn state).
    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let RelicData::PaelsEye(data) = &mut ctx.state().relics[me].data else {
            unreachable!("a PaelsEye has PaelsEye data");
        };
        data.was_owner_part_of_last_player_turn = false;
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsEye {
    type Me = RelicId;

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if side != Side::Player || data(eng.s, me).used_this_combat {
            return Ok(());
        }
        let part = participants.contains(&eng.s.player.creature);
        data_mut(eng, me).was_owner_part_of_last_player_turn = part;
        Ok(())
    }

    fn should_take_extra_turn(&self, s: &CombatState, me: RelicId) -> bool {
        // `player == Owner`: the only player.
        triggers(s, me)
    }

    fn before_side_turn_end_early(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&eng.s.player.creature) || !triggers(eng.s, me) {
            return Ok(());
        }
        for card in eng.s.player.hand.clone() {
            eng.exhaust(card, false)?;
        }
        Ok(())
    }

    fn after_taking_extra_turn(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng, me).used_this_combat = true;
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng, me).used_this_combat = false;
        Ok(())
    }
}
