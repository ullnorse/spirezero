//! `LouseProgenitor` (`Models.Monsters/LouseProgenitor.cs`): `CurlUp` (block) when added to the
//! room. Web Cannon (attack, 2 Frail), Curl and Grow (block, +Strength), Pounce (attack) in a
//! cycle, opening with Web Cannon. `Curled` (set by Curl and Grow and by `CurlUpPower`, cleared by
//! the next attack) only picks animations, so it is not kept.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const WEB_CANNON: StateIdx = 0;
const CURL_AND_GROW: StateIdx = 1;
const POUNCE: StateIdx = 2;

/// `_webFrail`.
const WEB_FRAIL: i32 = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "WEB_CANNON_MOVE",
            follow_up: CURL_AND_GROW,
        },
        StateDef::Move {
            id: "CURL_AND_GROW_MOVE",
            follow_up: POUNCE,
        },
        StateDef::Move {
            id: "POUNCE_MOVE",
            follow_up: WEB_CANNON,
        },
    ],
    initial: WEB_CANNON,
};

/// `CurlBlock`.
const fn curl_block(ascension: u8) -> i32 {
    if_ascension(ascension, TOUGH_ENEMIES, 18, 14)
}

/// `LouseProgenitor`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LouseProgenitor;

#[sts2_macros::hooks]
impl Hooks for LouseProgenitor {
    type Me = CreatureId;
}

impl MonsterModel for LouseProgenitor {
    fn name(&self) -> &'static str {
        "Louse Progenitor"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 138, 134)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 141, 136)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            WEB_CANNON => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 10, 9))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(
                    &targets,
                    PowerKind::Frail,
                    Decimal::from(WEB_FRAIL),
                    Some(me),
                )
            }
            CURL_AND_GROW => {
                eng.gain_block(
                    me,
                    Decimal::from(curl_block(ascension)),
                    ValueProps::MOVE,
                    None,
                )?;
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 7, 5);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            POUNCE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14)),
            _ => unreachable!("not a LouseProgenitor move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let block = curl_block(eng.s.ascension);
        eng.apply_power(me, PowerKind::CurlUp, Decimal::from(block), Some(me))
    }
}
