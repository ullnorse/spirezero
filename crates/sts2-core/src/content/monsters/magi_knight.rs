//! `MagiKnight` (`Models.Monsters/MagiKnight.cs`): Power Shield (attack, block) -> Dampen (`Dampen`
//! on each player, with this knight as a caster) -> Ram (attack) -> Prep (block) -> Magic Bomb
//! (attack) -> Ram -> ... The bomb's positioning and the banter are cosmetic.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::dampen;
use crate::decimal::Decimal;

const POWER_SHIELD: StateIdx = 0;
const DAMPEN: StateIdx = 1;
const RAM: StateIdx = 2;
const PREP: StateIdx = 3;
const MAGIC_BOMB: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "POWER_SHIELD_MOVE",
            follow_up: DAMPEN,
        },
        StateDef::Move {
            id: "DAMPEN_MOVE",
            follow_up: RAM,
        },
        StateDef::Move {
            id: "RAM_MOVE",
            follow_up: PREP,
        },
        StateDef::Move {
            id: "PREP_MOVE",
            follow_up: MAGIC_BOMB,
        },
        StateDef::Move {
            id: "MAGIC_BOMB",
            follow_up: RAM,
        },
    ],
    initial: POWER_SHIELD,
};

/// `MagiKnight`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MagiKnight;

/// `PowerShieldBlock`.
const fn power_shield_block(ascension: u8) -> i32 {
    if_ascension(ascension, TOUGH_ENEMIES, 9, 5)
}

#[sts2_macros::hooks]
impl Hooks for MagiKnight {
    type Me = CreatureId;
}

impl MonsterModel for MagiKnight {
    fn name(&self) -> &'static str {
        "Magi Knight"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 89, 82)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        let block = Decimal::from(power_shield_block(ascension));
        match state {
            POWER_SHIELD => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6))?;
                eng.gain_block(me, block, ValueProps::MOVE, None)?;
                Ok(())
            }
            DAMPEN => {
                for target in eng.s.player_creatures() {
                    dampen::apply_from_caster(eng, target, me)?;
                }
                Ok(())
            }
            RAM => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 11, 10)),
            PREP => eng.gain_block(me, block, ValueProps::MOVE, None).map(drop),
            MAGIC_BOMB => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 40, 35)),
            _ => unreachable!("not a MagiKnight move: {state}"),
        }
    }
}
