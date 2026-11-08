//! `GremlinMerc` (`Models.Monsters/GremlinMerc.cs`): Gimme (2 hits) -> Double Smash (2 hits, Weak)
//! -> Hehe (attack, +2 Strength) -> Gimme ..., and after each attack every `ThieveryPower` steals
//! gold. It enters with `Surprise` and one `Thievery` (20) per player. The banter line is cosmetic.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::{PowerKind, thievery};
use crate::decimal::Decimal;

const GIMME: StateIdx = 0;
const DOUBLE_SMASH: StateIdx = 1;
const HEHE: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "GIMME_MOVE",
            follow_up: DOUBLE_SMASH,
        },
        StateDef::Move {
            id: "DOUBLE_SMASH_MOVE",
            follow_up: HEHE,
        },
        StateDef::Move {
            id: "HEHE_MOVE",
            follow_up: GIMME,
        },
    ],
    initial: GIMME,
};

/// `ThieveryPower`'s amount.
const THIEVERY_AMOUNT: i32 = 20;

/// `GremlinMerc`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GremlinMerc;

#[sts2_macros::hooks]
impl Hooks for GremlinMerc {
    type Me = CreatureId;
}

impl MonsterModel for GremlinMerc {
    fn name(&self) -> &'static str {
        "Gremlin Merc"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 51, 47)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 53, 49)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Surprise, Decimal::ONE, Some(me))?;
        for player in eng.s.player_creatures() {
            thievery::apply(eng, me, player, THIEVERY_AMOUNT)?;
        }
        Ok(())
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            // `GimmeRepeat` 2.
            GIMME => {
                attack_players_hits(eng, me, if_ascension(ascension, TOUGH_ENEMIES, 8, 7), 2)?;
                thievery::steal_all(eng, me);
                Ok(())
            }
            // `DoubleSmashRepeat` 2.
            DOUBLE_SMASH => {
                attack_players_hits(eng, me, if_ascension(ascension, TOUGH_ENEMIES, 7, 6), 2)?;
                thievery::steal_all(eng, me);
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::from(2), Some(me))
            }
            HEHE => {
                attack_players(eng, me, if_ascension(ascension, TOUGH_ENEMIES, 9, 8))?;
                thievery::steal_all(eng, me);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not a GremlinMerc move: {state}"),
        }
    }
}
