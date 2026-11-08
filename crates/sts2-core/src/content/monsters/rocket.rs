//! `Rocket` (`Models.Monsters/Rocket.cs`): the `KaiserCrabBoss`'s right arm. Added to the room, it
//! puts `SurroundedPower` on every opponent and takes `BackAttackRightPower` and `CrabRagePower`. A
//! fixed cycle: Targeting Reticle (attack) -> Precision Beam (attack) -> Charge Up (Strength) ->
//! Laser (attack) -> Recharge (nothing) -> Targeting Reticle. The hooks it overrides besides
//! `AfterAddedToRoom` only drive animations, sounds and music.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const TARGETING_RETICLE: StateIdx = 0;
const PRECISION_BEAM: StateIdx = 1;
const CHARGE_UP: StateIdx = 2;
const LASER: StateIdx = 3;
const RECHARGE: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "TARGETING_RETICLE_MOVE",
            follow_up: PRECISION_BEAM,
        },
        StateDef::Move {
            id: "PRECISION_BEAM_MOVE",
            follow_up: CHARGE_UP,
        },
        StateDef::Move {
            id: "CHARGE_UP_MOVE",
            follow_up: LASER,
        },
        StateDef::Move {
            id: "LASER_MOVE",
            follow_up: RECHARGE,
        },
        StateDef::Move {
            id: "RECHARGE_MOVE",
            follow_up: TARGETING_RETICLE,
        },
    ],
    initial: TARGETING_RETICLE,
};

/// `Rocket`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rocket;

#[sts2_macros::hooks]
impl Hooks for Rocket {
    type Me = CreatureId;
}

impl MonsterModel for Rocket {
    fn name(&self) -> &'static str {
        "Rocket"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 209, 199)
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
            TARGETING_RETICLE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 4, 3))
            }
            PRECISION_BEAM => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 20, 18))
            }
            CHARGE_UP => {
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(amount), Some(me))
            }
            LASER => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 35, 31)),
            RECHARGE => Ok(()),
            _ => unreachable!("not a Rocket move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        // `CombatState.GetOpponentsOf(Creature)`: the whole player side (Osty joins later).
        let opponents = eng.s.creatures_on(Side::Player);
        eng.apply_power_to_all(&opponents, PowerKind::Surrounded, Decimal::ONE, Some(me))?;
        eng.apply_power(me, PowerKind::BackAttackRight, Decimal::ONE, Some(me))?;
        eng.apply_power(me, PowerKind::CrabRage, Decimal::ONE, Some(me))
    }
}
