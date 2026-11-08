//! `OwlMagistrate` (`Models.Monsters/OwlMagistrate.cs`): Magistrate Scrutiny -> Peck Assault
//! (6 hits) -> Judicial Flight (gains Soar: takes half damage from powered attacks) -> Verdict
//! (damage, 4 Vulnerable, loses Soar) -> Magistrate Scrutiny. `_isFlying` only picks sounds.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const MAGISTRATE_SCRUTINY: StateIdx = 0;
const PECK_ASSAULT: StateIdx = 1;
const JUDICIAL_FLIGHT: StateIdx = 2;
const VERDICT: StateIdx = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "MAGISTRATE_SCRUTINY",
            follow_up: PECK_ASSAULT,
        },
        StateDef::Move {
            id: "PECK_ASSAULT",
            follow_up: JUDICIAL_FLIGHT,
        },
        StateDef::Move {
            id: "JUDICIAL_FLIGHT",
            follow_up: VERDICT,
        },
        StateDef::Move {
            id: "VERDICT",
            follow_up: MAGISTRATE_SCRUTINY,
        },
    ],
    initial: MAGISTRATE_SCRUTINY,
};

/// `OwlMagistrate`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OwlMagistrate;

#[sts2_macros::hooks]
impl Hooks for OwlMagistrate {
    type Me = CreatureId;
}

impl MonsterModel for OwlMagistrate {
    fn name(&self) -> &'static str {
        "Owl Magistrate"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 247, 231)
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
            MAGISTRATE_SCRUTINY => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 17, 16))
            }
            PECK_ASSAULT => attack_players_hits(eng, me, 4, 6),
            JUDICIAL_FLIGHT => eng.apply_power(me, PowerKind::Soar, Decimal::ONE, Some(me)),
            VERDICT => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 36, 33))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(
                    &targets,
                    PowerKind::Vulnerable,
                    Decimal::from(4),
                    Some(me),
                )?;
                let soar = eng
                    .s
                    .creature(me)
                    .powers
                    .iter()
                    .find(|p| p.kind == PowerKind::Soar)
                    .map(|p| p.id);
                if let Some(soar) = soar {
                    eng.remove_power(me, soar)?;
                }
                Ok(())
            }
            _ => unreachable!("not an OwlMagistrate move: {state}"),
        }
    }
}
