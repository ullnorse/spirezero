//! `Guardbot` (`Models.Monsters/Guardbot.cs`): a `Fabricator` bot that Guards every turn: 15
//! unpowered block on each `Fabricator`. The fall position is cosmetic.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{MonsterKind, MonsterModel, TOUGH_ENEMIES, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::decimal::Decimal;

const GUARD: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "GUARD_MOVE",
        follow_up: GUARD,
    }],
    initial: GUARD,
};

/// `Guardbot`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Guardbot;

#[sts2_macros::hooks]
impl Hooks for Guardbot {
    type Me = CreatureId;
}

impl MonsterModel for Guardbot {
    fn name(&self) -> &'static str {
        "Guardbot"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 17, 16)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 21, 20)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, _me: CreatureId, state: StateIdx) -> Flow {
        match state {
            GUARD => {
                let fabricators: Vec<CreatureId> = eng
                    .s
                    .enemies
                    .iter()
                    .copied()
                    .filter(|&e| {
                        eng.s
                            .creature(e)
                            .monster
                            .as_ref()
                            .is_some_and(|m| matches!(m.kind, MonsterKind::Fabricator(_)))
                    })
                    .collect();
                for fabricator in fabricators {
                    eng.gain_block(fabricator, Decimal::from(15), ValueProps::UNPOWERED, None)?;
                }
                Ok(())
            }
            _ => unreachable!("not a Guardbot move: {state}"),
        }
    }
}
