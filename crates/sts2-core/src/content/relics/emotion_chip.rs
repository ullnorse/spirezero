//! `EmotionChip` (`Models.Relics/EmotionChip.cs`): at the start of each of the owner's turns, if it
//! lost HP during its previous turn (a `DamageReceivedEntry` for the owner that was not fully
//! blocked and `HappenedLastPlayerTurn`), every orb triggers its passive, counting the
//! trigger-count hooks (`GoldPlatedCables`). The combat history keeps only the current turn, so
//! the relic remembers the player turn number of the latest such entry; it is reset when a combat
//! room is entered (the history is cleared per combat). `Status` is presentation.

use crate::combat::hooks::{DamageEvent, Hooks};
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::{RelicData, RelicModel};

pub struct EmotionChip;

/// `EmotionChip`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// The player turn number of the latest `DamageReceivedEntry` for the owner that was not
    /// fully blocked, this combat.
    pub last_unblocked_hit_turn: Option<u32>,
}

impl RelicModel for EmotionChip {
    fn name(&self) -> &'static str {
        "Emotion Chip"
    }

    fn init_data(&self) -> RelicData {
        RelicData::EmotionChip(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for EmotionChip {
    type Me = RelicId;

    /// `AfterRoomEntered` of a `CombatRoom` (the engine only dispatches it for fights).
    fn after_room_entered(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        eng.s.relic_mut(me).data = RelicData::EmotionChip(Data::default());
        Ok(())
    }

    /// Records the entry `CombatHistory.DamageReceived` made for this hit (every result of a
    /// combat in progress, `CreatureCmd.cs:316-320`).
    fn after_damage_received(&self, eng: &mut Engine<'_>, me: RelicId, hit: DamageEvent) -> Flow {
        if hit.target == eng.s.player.creature && !hit.result.was_fully_blocked {
            let turn = eng.s.player.turn_number;
            eng.s.relic_mut(me).data = RelicData::EmotionChip(Data {
                last_unblocked_hit_turn: Some(turn),
            });
        }
        Ok(())
    }

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        let RelicData::EmotionChip(data) = eng.s.relic(me).data else {
            unreachable!("an EmotionChip has EmotionChip data");
        };
        // `HappenedLastPlayerTurn`: the entry's turn is `TurnNumber - 1`.
        let turn = eng.s.player.turn_number;
        if turn == 0 || data.last_unblocked_hit_turn != Some(turn - 1) {
            return Ok(());
        }
        for orb in eng.s.player.orbs.clone() {
            eng.orb_passive(orb, None, true)?;
        }
        Ok(())
    }
}
