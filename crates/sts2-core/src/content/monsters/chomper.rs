//! `Chomper` (`Models.Monsters/Chomper.cs`): 2 Artifact when added to the room. Clamp (2 hits) and
//! Screech (3 `Dazed` into the discard pile) alternate; the encounter's second Chomper has
//! `ScreamFirst` and opens with Screech.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, add_status_cards, attack_players_hits,
    if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const CLAMP: StateIdx = 0;
const SCREECH: StateIdx = 1;

/// `_screechStatusCount`.
const SCREECH_STATUS_COUNT: u32 = 3;
/// `_clampRepeat`.
const CLAMP_REPEAT: u32 = 2;

const STATES: &[StateDef] = &[
    StateDef::Move {
        id: "CLAMP_MOVE",
        follow_up: SCREECH,
    },
    StateDef::Move {
        id: "SCREECH_MOVE",
        follow_up: CLAMP,
    },
];

static MOVES: MoveTable = MoveTable {
    states: STATES,
    initial: CLAMP,
};

static MOVES_SCREAM_FIRST: MoveTable = MoveTable {
    states: STATES,
    initial: SCREECH,
};

/// Per-combat fields, set by the encounter before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Chomper {
    /// `ScreamFirst`.
    pub scream_first: bool,
}

#[sts2_macros::hooks]
impl Hooks for Chomper {
    type Me = CreatureId;
}

impl MonsterModel for Chomper {
    fn name(&self) -> &'static str {
        "Chomper"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 63, 60)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 67, 64)
    }

    fn moves(&self) -> &'static MoveTable {
        if self.scream_first {
            &MOVES_SCREAM_FIRST
        } else {
            &MOVES
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            CLAMP => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 9, 8),
                CLAMP_REPEAT,
            ),
            // `TalkCmd.Play` is cosmetic.
            SCREECH => add_status_cards(eng, CardKind::Dazed, SCREECH_STATUS_COUNT),
            _ => unreachable!("not a Chomper move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Artifact, Decimal::from(2), Some(me))
    }
}
