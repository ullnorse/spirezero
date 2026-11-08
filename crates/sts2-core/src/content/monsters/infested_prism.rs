//! `InfestedPrism` (`Models.Monsters/InfestedPrism.cs`): enters with `VitalSpark`; Jab ->
//! Radiate (attack and block) -> Whirlwind (3 hits) -> Pulsate (attack, block and more
//! `VitalSpark`) -> Jab.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const JAB: StateIdx = 0;
const RADIATE: StateIdx = 1;
const WHIRLWIND: StateIdx = 2;
const PULSATE: StateIdx = 3;

/// `WhirlwindRepeat`.
const WHIRLWIND_REPEAT: u32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "JAB_MOVE",
            follow_up: RADIATE,
        },
        StateDef::Move {
            id: "RADIATE_MOVE",
            follow_up: WHIRLWIND,
        },
        StateDef::Move {
            id: "WHIRLWIND_MOVE",
            follow_up: PULSATE,
        },
        StateDef::Move {
            id: "PULSATE_MOVE",
            follow_up: JAB,
        },
    ],
    initial: JAB,
};

/// `InfestedPrism`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InfestedPrism;

/// `VitalSparkAmount`.
fn vital_spark_amount(ascension: u8) -> Decimal {
    Decimal::from(if_ascension(ascension, DEADLY_ENEMIES, 3, 2))
}

#[sts2_macros::hooks]
impl Hooks for InfestedPrism {
    type Me = CreatureId;
}

impl MonsterModel for InfestedPrism {
    fn name(&self) -> &'static str {
        "Infested Prism"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 171, 161)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = vital_spark_amount(eng.s.ascension);
        eng.apply_power(me, PowerKind::VitalSpark, amount, Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            JAB => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 17, 15)),
            RADIATE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 13, 11))?;
                let block = if_ascension(ascension, DEADLY_ENEMIES, 13, 11);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                Ok(())
            }
            WHIRLWIND => attack_players_hits(
                eng,
                me,
                if_ascension(ascension, DEADLY_ENEMIES, 6, 5),
                WHIRLWIND_REPEAT,
            ),
            PULSATE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 10, 8))?;
                let block = if_ascension(ascension, TOUGH_ENEMIES, 22, 20);
                eng.gain_block(me, Decimal::from(block), ValueProps::MOVE, None)?;
                eng.apply_power(
                    me,
                    PowerKind::VitalSpark,
                    vital_spark_amount(ascension),
                    Some(me),
                )
            }
            _ => unreachable!("not an InfestedPrism move: {state}"),
        }
    }
}
