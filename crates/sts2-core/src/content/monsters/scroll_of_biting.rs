//! `ScrollOfBiting` (`Models.Monsters/ScrollOfBiting.cs`): starts with 2 `PaperCuts`; Chomp
//! (attack) -> More Teeth (+2 Strength) -> Chew (2 hits) -> a random branch over Chomp (never
//! twice in a row) and Chew (at most twice in a row). The encounter picks the first move
//! (`StarterMoveIdx % 3`: Chomp, Chew, More Teeth). The skin (`Rng.Chaotic`) is cosmetic.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const CHOMP: StateIdx = 0;
const CHEW: StateIdx = 1;
const MORE_TEETH: StateIdx = 2;
const RAND: StateIdx = 3;

const STATES: &[StateDef] = &[
    StateDef::Move {
        id: "CHOMP",
        follow_up: MORE_TEETH,
    },
    StateDef::Move {
        id: "CHEW",
        follow_up: RAND,
    },
    StateDef::Move {
        id: "MORE_TEETH",
        follow_up: CHEW,
    },
    StateDef::Random {
        id: "rand",
        branches: &[
            Branch::new(CHOMP, Repeat::CannotRepeat),
            Branch::new(CHEW, Repeat::CanRepeatXTimes(2)),
        ],
    },
];

static MOVES_CHOMP: MoveTable = MoveTable {
    states: STATES,
    initial: CHOMP,
};
static MOVES_CHEW: MoveTable = MoveTable {
    states: STATES,
    initial: CHEW,
};
static MOVES_MORE_TEETH: MoveTable = MoveTable {
    states: STATES,
    initial: MORE_TEETH,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ScrollOfBiting {
    /// `StarterMoveIdx`, set by the encounter.
    pub starter_move_idx: i32,
}

#[sts2_macros::hooks]
impl Hooks for ScrollOfBiting {
    type Me = CreatureId;
}

impl MonsterModel for ScrollOfBiting {
    fn name(&self) -> &'static str {
        "Scroll of Biting"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 33, 30)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 39, 37)
    }

    fn moves(&self) -> &'static MoveTable {
        match self.starter_move_idx % 3 {
            0 => &MOVES_CHOMP,
            1 => &MOVES_CHEW,
            _ => &MOVES_MORE_TEETH,
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::PaperCuts, Decimal::from(2), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            CHOMP => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14)),
            CHEW => attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 6, 5), 2),
            MORE_TEETH => eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me)),
            _ => unreachable!("not a ScrollOfBiting move: {state}"),
        }
    }
}
