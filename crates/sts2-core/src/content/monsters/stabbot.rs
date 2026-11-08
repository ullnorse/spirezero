//! `Stabbot` (`Models.Monsters/Stabbot.cs`): a `Fabricator` bot that Stabs (attack, 1 Frail)
//! every turn. The fall position is cosmetic.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const STAB: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "STAB_MOVE",
        follow_up: STAB,
    }],
    initial: STAB,
};

/// `Stabbot`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stabbot;

#[sts2_macros::hooks]
impl Hooks for Stabbot {
    type Me = CreatureId;
}

impl MonsterModel for Stabbot {
    fn name(&self) -> &'static str {
        "Stabbot"
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

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            STAB => {
                attack_players(
                    eng,
                    me,
                    if_ascension(eng.s.ascension, DEADLY_ENEMIES, 12, 11),
                )?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::ONE, Some(me))
            }
            _ => unreachable!("not a Stabbot move: {state}"),
        }
    }
}
