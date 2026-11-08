//! `MechaKnight` (`Models.Monsters/MechaKnight.cs`): starts with 3 `Artifact`; Charge (attack)
//! -> Flamethrower (attack, 4 `Burn` into the hand) -> Windup (15 block, +5 Strength) -> Heavy
//! Cleave (attack) -> Flamethrower -> ... `IsWoundUp` only picks the hurt animation.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType, ValueProps};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const CHARGE: StateIdx = 0;
const HEAVY_CLEAVE: StateIdx = 1;
const WINDUP: StateIdx = 2;
const FLAMETHROWER: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "CHARGE_MOVE",
            follow_up: FLAMETHROWER,
        },
        StateDef::Move {
            id: "HEAVY_CLEAVE_MOVE",
            follow_up: FLAMETHROWER,
        },
        StateDef::Move {
            id: "WINDUP_MOVE",
            follow_up: HEAVY_CLEAVE,
        },
        StateDef::Move {
            id: "FLAMETHROWER_MOVE",
            follow_up: WINDUP,
        },
    ],
    initial: CHARGE,
};

/// `_flamethrowerCardCount`.
const FLAMETHROWER_CARD_COUNT: u32 = 4;
/// `_windupBlock`.
const WINDUP_BLOCK: i32 = 15;

/// `MechaKnight`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MechaKnight;

#[sts2_macros::hooks]
impl Hooks for MechaKnight {
    type Me = CreatureId;
}

impl MonsterModel for MechaKnight {
    fn name(&self) -> &'static str {
        "Mecha Knight"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 320, 300)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Artifact, Decimal::from(3), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            CHARGE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 30, 25)),
            HEAVY_CLEAVE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 40, 35))
            }
            WINDUP => {
                eng.gain_block(me, Decimal::from(WINDUP_BLOCK), ValueProps::MOVE, None)?;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(5), Some(me))
            }
            FLAMETHROWER => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 12, 8))?;
                for target in eng.s.player_creatures() {
                    eng.add_to_combat_and_preview(
                        target,
                        CardKind::Burn,
                        PileType::Hand,
                        FLAMETHROWER_CARD_COUNT,
                        None,
                    )?;
                }
                Ok(())
            }
            _ => unreachable!("not a MechaKnight move: {state}"),
        }
    }
}
