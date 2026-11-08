//! `Vantom` (`Models.Monsters/Vantom.cs`): gains Slippery when added to the room (9 from the
//! `ToughEnemies` ascension, else 8). Ink Blot, Inky Lance (2 hits), Dismember (3 `Wound` into the
//! discard pile) and Prepare (+2 Strength), in a cycle. `ShouldDisappearFromDoom` only picks the
//! Doom animation; the music, animation and scaling code is UI only.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, add_status_cards, attack_players,
    attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `_inkyLanceRepeat`.
const INKY_LANCE_REPEAT: u32 = 2;
/// `_dismemberWounds`.
const DISMEMBER_WOUNDS: u32 = 3;
/// `_prepareStrength`.
const PREPARE_STRENGTH: i32 = 2;

const INK_BLOT: StateIdx = 0;
const INKY_LANCE: StateIdx = 1;
const DISMEMBER: StateIdx = 2;
const PREPARE: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "INK_BLOT_MOVE",
            follow_up: INKY_LANCE,
        },
        StateDef::Move {
            id: "INKY_LANCE_MOVE",
            follow_up: DISMEMBER,
        },
        StateDef::Move {
            id: "DISMEMBER_MOVE",
            follow_up: PREPARE,
        },
        StateDef::Move {
            id: "PREPARE_MOVE",
            follow_up: INK_BLOT,
        },
    ],
    initial: INK_BLOT,
};

/// `Vantom`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Vantom;

#[sts2_macros::hooks]
impl Hooks for Vantom {
    type Me = CreatureId;
}

impl MonsterModel for Vantom {
    fn name(&self) -> &'static str {
        "Vantom"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 183, 173)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            INK_BLOT => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7)),
            INKY_LANCE => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 7, 6),
                INKY_LANCE_REPEAT,
            ),
            DISMEMBER => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 30, 26))?;
                add_status_cards(eng, CardKind::Wound, DISMEMBER_WOUNDS)
            }
            PREPARE => eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(PREPARE_STRENGTH),
                Some(me),
            ),
            _ => unreachable!("not a Vantom move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = if_ascension(eng.s.ascension, TOUGH_ENEMIES, 9, 8);
        eng.apply_power(me, PowerKind::Slippery, Decimal::from(amount), Some(me))
    }
}
