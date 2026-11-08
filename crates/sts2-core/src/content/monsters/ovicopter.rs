//! `Ovicopter` (`Models.Monsters/Ovicopter.cs`): opens with Lay Eggs (three `ToughEgg`s, each a
//! `Minion`, into the last free slots), then Smash, then Tenderizer (attack, 2 Vulnerable), then
//! Lay Eggs again while at most three of its side are alive (`CanLay`), else Nutritional Paste
//! (Strength), each followed by Smash. The idle loop and lay sounds are audio only.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, ToughEgg};
use super::{attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const NUTRITIONAL_PASTE: StateIdx = 0;
const LAY_EGGS: StateIdx = 1;
const SMASH: StateIdx = 2;
const TENDERIZER: StateIdx = 3;
const SUMMON_BRANCH: StateIdx = 4;

/// Eggs per Lay Eggs.
const EGGS: usize = 3;

/// `CanLay`: at most three living creatures on its side, itself included.
fn can_lay(s: &CombatState, me: CreatureId) -> bool {
    let side = s.creature(me).side;
    s.creatures_on(side)
        .into_iter()
        .filter(|&c| s.creature(c).is_alive())
        .count()
        <= 3
}

fn cannot_lay(s: &CombatState, me: CreatureId) -> bool {
    !can_lay(s, me)
}

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "NUTRITIONAL_PASTE_MOVE",
            follow_up: SMASH,
        },
        StateDef::Move {
            id: "LAY_EGGS_MOVE",
            follow_up: SMASH,
        },
        StateDef::Move {
            id: "SMASH_MOVE",
            follow_up: TENDERIZER,
        },
        StateDef::Move {
            id: "TENDERIZER_MOVE",
            follow_up: SUMMON_BRANCH,
        },
        StateDef::Conditional {
            id: "SUMMON_BRANCH_STATE",
            branches: &[(LAY_EGGS, can_lay), (NUTRITIONAL_PASTE, cannot_lay)],
        },
    ],
    initial: LAY_EGGS,
};

/// `Ovicopter`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ovicopter;

#[sts2_macros::hooks]
impl Hooks for Ovicopter {
    type Me = CreatureId;
}

impl MonsterModel for Ovicopter {
    fn name(&self) -> &'static str {
        "Ovicopter"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 126, 124)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 132, 130)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            LAY_EGGS => {
                for _ in 0..EGGS {
                    // `Slots.LastOrDefault(s => Enemies.All(c => c.SlotName != s))`.
                    let free = eng.s.encounter.slots().iter().rev().copied().find(|&slot| {
                        eng.s
                            .enemies
                            .iter()
                            .all(|&e| eng.s.creature(e).slot != Some(slot))
                    });
                    if free.is_some() {
                        let egg = eng.add_enemy(MonsterKind::ToughEgg(ToughEgg), free)?;
                        eng.apply_power(egg, PowerKind::Minion, Decimal::ONE, Some(me))?;
                    }
                }
                Ok(())
            }
            NUTRITIONAL_PASTE => {
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 4, 3);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(amount), Some(me))
            }
            SMASH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 17, 16)),
            TENDERIZER => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Vulnerable, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not an Ovicopter move: {state}"),
        }
    }
}
