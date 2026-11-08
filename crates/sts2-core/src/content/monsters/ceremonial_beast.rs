//! `CeremonialBeast` (`Models.Monsters/CeremonialBeast.cs`): Stamp (applies Plow to itself),
//! then Plow (attack, +2 Strength) forever, until `PlowPower` stuns it: then `STUNNED`, and the
//! second phase cycles Beast Cry (applies Ringing to the player), Stomp (attack) and Crush
//! (attack, +Strength). The `STUN_MOVE` state is never entered (`CreatureCmd.Stun` builds its own
//! `STUNNED` move). `IsStunnedByPlowRemoval`, `IsInSecondPhase` and `InMidCharge` are UI only, as
//! are `ShouldDisappearFromDoom` and `ShouldFadeAfterDeath`.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// `PlowStrength`.
const PLOW_STRENGTH: i32 = 2;

const PLOW: StateIdx = 0;
const STAMP: StateIdx = 1;
const BEAST_CRY: StateIdx = 2;
const STOMP: StateIdx = 3;
const CRUSH: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "PLOW_MOVE",
            follow_up: PLOW,
        },
        StateDef::Move {
            id: "STAMP_MOVE",
            follow_up: PLOW,
        },
        StateDef::Move {
            id: "BEAST_CRY_MOVE",
            follow_up: STOMP,
        },
        StateDef::Move {
            id: "STOMP_MOVE",
            follow_up: CRUSH,
        },
        StateDef::Move {
            id: "CRUSH_MOVE",
            follow_up: BEAST_CRY,
        },
    ],
    initial: STAMP,
};

/// `CeremonialBeast`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CeremonialBeast;

impl CeremonialBeast {
    /// The state `BeastCryState` (the follow-up of the stun).
    pub const BEAST_CRY: StateIdx = BEAST_CRY;
}

#[sts2_macros::hooks]
impl Hooks for CeremonialBeast {
    type Me = CreatureId;
}

impl MonsterModel for CeremonialBeast {
    fn name(&self) -> &'static str {
        "Ceremonial Beast"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 262, 252)
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
            STAMP => {
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 160, 150);
                eng.apply_power(me, PowerKind::Plow, Decimal::from(amount), Some(me))
            }
            PLOW => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 20, 18))?;
                eng.apply_power(
                    me,
                    PowerKind::Strength,
                    Decimal::from(PLOW_STRENGTH),
                    Some(me),
                )
            }
            BEAST_CRY => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Ringing, Decimal::ONE, Some(me))
            }
            STOMP => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 17, 15)),
            CRUSH => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 19, 17))?;
                let strength = if_ascension(ascension, DEADLY_ENEMIES, 4, 3);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))
            }
            _ => unreachable!("not a CeremonialBeast move: {state}"),
        }
    }
}
