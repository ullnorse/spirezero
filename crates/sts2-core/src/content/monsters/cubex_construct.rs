//! `CubexConstruct` (`Models.Monsters/CubexConstruct.cs`): starts with 13 Block and 1 Artifact.
//! Charges up (2 Strength), then Repeater Blast (attack, 2 Strength) twice, then Expel (two
//! hits), then Repeater Blast again. `_isBurrowed`/`_isCharging` only drive the animator.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const CHARGE_UP: StateIdx = 0;
const REPEATER_BLAST: StateIdx = 1;
const REPEATER_BLAST_2: StateIdx = 2;
const EXPEL: StateIdx = 3;

/// `_expelRepeats`.
const EXPEL_REPEATS: u32 = 2;

/// The Block gained in `AfterAddedToRoom`.
const STARTING_BLOCK: i32 = 13;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "CHARGE_UP_MOVE",
            follow_up: REPEATER_BLAST,
        },
        StateDef::Move {
            id: "REPEATER_BLAST_MOVE",
            follow_up: REPEATER_BLAST_2,
        },
        StateDef::Move {
            id: "REPEATER_BLAST_MOVE_2",
            follow_up: EXPEL,
        },
        StateDef::Move {
            id: "EXPEL_MOVE",
            follow_up: REPEATER_BLAST,
        },
    ],
    initial: CHARGE_UP,
};

/// `CubexConstruct`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CubexConstruct;

#[sts2_macros::hooks]
impl Hooks for CubexConstruct {
    type Me = CreatureId;
}

impl MonsterModel for CubexConstruct {
    fn name(&self) -> &'static str {
        "Cubex Construct"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 70, 65)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        let gain_strength = |eng: &mut Engine<'_>| {
            eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me))
        };
        match state {
            CHARGE_UP => gain_strength(eng),
            REPEATER_BLAST | REPEATER_BLAST_2 => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7))?;
                gain_strength(eng)
            }
            EXPEL => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 6, 5),
                EXPEL_REPEATS,
            ),
            _ => unreachable!("not a Cubex Construct move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.gain_block(me, Decimal::from(STARTING_BLOCK), ValueProps::MOVE, None)?;
        eng.apply_power(me, PowerKind::Artifact, Decimal::ONE, Some(me))
    }
}
