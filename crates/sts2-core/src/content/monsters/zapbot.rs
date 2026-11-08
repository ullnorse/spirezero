//! `Zapbot` (`Models.Monsters/Zapbot.cs`): a `Fabricator` bot with 2 `HighVoltage` that Zaps
//! (attack) every turn. The fall position is cosmetic.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const ZAP: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "ZAP",
        follow_up: ZAP,
    }],
    initial: ZAP,
};

/// `Zapbot`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Zapbot;

#[sts2_macros::hooks]
impl Hooks for Zapbot {
    type Me = CreatureId;
}

impl MonsterModel for Zapbot {
    fn name(&self) -> &'static str {
        "Zapbot"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 19, 18)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 24, 23)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::HighVoltage, Decimal::from(2), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            ZAP => attack_players(
                eng,
                me,
                if_ascension(eng.s.ascension, DEADLY_ENEMIES, 15, 14),
            ),
            _ => unreachable!("not a Zapbot move: {state}"),
        }
    }
}
