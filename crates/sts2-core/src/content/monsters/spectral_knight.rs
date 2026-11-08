//! `SpectralKnight` (`Models.Monsters/SpectralKnight.cs`): Hex (2 `Hex` on each player) ->
//! Soul Slash (attack) -> a random branch over Soul Slash (at most twice in a row) and Soul Flame
//! (3 hits, never twice in a row).

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const HEX: StateIdx = 0;
const SOUL_SLASH: StateIdx = 1;
const SOUL_FLAME: StateIdx = 2;
const RAND: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "HEX",
            follow_up: SOUL_SLASH,
        },
        StateDef::Move {
            id: "SOUL_SLASH",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "SOUL_FLAME",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::new(SOUL_SLASH, Repeat::CanRepeatXTimes(2)),
                Branch::new(SOUL_FLAME, Repeat::CannotRepeat),
            ],
        },
    ],
    initial: HEX,
};

/// `SpectralKnight`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpectralKnight;

#[sts2_macros::hooks]
impl Hooks for SpectralKnight {
    type Me = CreatureId;
}

impl MonsterModel for SpectralKnight {
    fn name(&self) -> &'static str {
        "Spectral Knight"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 97, 93)
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
            HEX => {
                for target in eng.s.player_creatures() {
                    eng.apply_power(target, PowerKind::Hex, Decimal::from(2), Some(me))?;
                }
                Ok(())
            }
            SOUL_SLASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 17, 15)),
            SOUL_FLAME => {
                attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 4, 3), 3)
            }
            _ => unreachable!("not a SpectralKnight move: {state}"),
        }
    }
}
