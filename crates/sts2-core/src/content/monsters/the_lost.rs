//! `TheLost` (`Models.Monsters/TheLost.cs`): starts with Possess Strength. Debilitating Smog
//! (steals 2 Strength from the player) -> Eye Lasers (2 hits) -> Debilitating Smog.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const DEBILITATING_SMOG: StateIdx = 0;
const EYE_LASERS: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "DEBILITATING_SMOG",
            follow_up: EYE_LASERS,
        },
        StateDef::Move {
            id: "EYE_LASERS",
            follow_up: DEBILITATING_SMOG,
        },
    ],
    initial: DEBILITATING_SMOG,
};

/// `DebilitatingSmogStrengthStealAmount` (2 at every ascension).
const SMOG_STRENGTH_STEAL: i32 = 2;

/// `TheLost`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TheLost;

#[sts2_macros::hooks]
impl Hooks for TheLost {
    type Me = CreatureId;
}

impl MonsterModel for TheLost {
    fn name(&self) -> &'static str {
        "The Lost"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 99, 93)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::PossessStrength, Decimal::ONE, None)
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            DEBILITATING_SMOG => {
                let targets = eng.s.player_creatures();
                let steal = Decimal::from(SMOG_STRENGTH_STEAL);
                eng.apply_power_to_all(
                    &targets,
                    PowerKind::Strength,
                    Decimal::ZERO - steal,
                    Some(me),
                )?;
                eng.apply_power(me, PowerKind::Strength, steal, Some(me))
            }
            EYE_LASERS => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 5, 4);
                attack_players_hits(eng, me, damage, 2)
            }
            _ => unreachable!("not a TheLost move: {state}"),
        }
    }
}
