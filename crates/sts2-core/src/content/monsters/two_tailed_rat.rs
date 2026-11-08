//! `TwoTailedRat` (`Models.Monsters/TwoTailedRat.cs`): a random branch over Scratch (attack), Disease
//! Bite (attack), Screech (Frail) and Call for Backup, which summons another rat into the last free
//! slot. The encounter gives each rat a `StarterMoveIndex` (`% 3`: Scratch, Disease Bite, Screech)
//! that replaces the random opening; -1 opens with the branch. Call for Backup needs two moves done
//! (`TurnsUntilSummonable`), fewer than three calls so far (`CallForBackupCount`, shared by the
//! rats), a free slot and no other rat about to call. While it is possible the other branches weigh
//! 1/12 each and Call for Backup 0.75; otherwise 1, 1, 1 and 0. Screech has a cooldown of 3 moves.
//! The skins and `Rng.Chaotic` picks in `SetupSkins` are cosmetic.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension,
    update_kind,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const RAND: StateIdx = 0;
const SCRATCH: StateIdx = 1;
const DISEASE_BITE: StateIdx = 2;
const SCREECH: StateIdx = 3;
const CALL_FOR_BACKUP: StateIdx = 4;

/// `CALL_FOR_BACKUP_MOVE`.
const CALL_FOR_BACKUP_MOVE: &str = "CALL_FOR_BACKUP_MOVE";
/// `_callForBackupLimit`.
const CALL_FOR_BACKUP_LIMIT: i32 = 3;
/// `1f / 12f`.
const ONE_TWELFTH: f32 = 0.083_333_336;
/// `_callForBackupChance`.
const CALL_FOR_BACKUP_CHANCE: f32 = 0.75;

/// `(!CanSummon()) ? 1f : (1f / 12f)`.
fn plain_weight(s: &CombatState, me: CreatureId) -> f32 {
    if can_summon(s, me) { ONE_TWELFTH } else { 1.0 }
}

/// `(!CanSummon()) ? 0f : 0.75f`.
fn call_for_backup_weight(s: &CombatState, me: CreatureId) -> f32 {
    if can_summon(s, me) {
        CALL_FOR_BACKUP_CHANCE
    } else {
        0.0
    }
}

const STATES: &[StateDef] = &[
    StateDef::Random {
        id: "RAND",
        branches: &[
            Branch::dynamic(SCRATCH, 0, Repeat::CannotRepeat, plain_weight),
            Branch::dynamic(DISEASE_BITE, 0, Repeat::CannotRepeat, plain_weight),
            Branch::dynamic(SCREECH, 3, Repeat::CannotRepeat, plain_weight),
            Branch::dynamic(
                CALL_FOR_BACKUP,
                0,
                Repeat::UseOnlyOnce,
                call_for_backup_weight,
            ),
        ],
    },
    StateDef::Move {
        id: "SCRATCH_MOVE",
        follow_up: RAND,
    },
    StateDef::Move {
        id: "DISEASE_BITE_MOVE",
        follow_up: RAND,
    },
    StateDef::Move {
        id: "SCREECH_MOVE",
        follow_up: RAND,
    },
    StateDef::Move {
        id: CALL_FOR_BACKUP_MOVE,
        follow_up: RAND,
    },
];

static MOVES_RAND: MoveTable = MoveTable {
    states: STATES,
    initial: RAND,
};
static MOVES_SCRATCH: MoveTable = MoveTable {
    states: STATES,
    initial: SCRATCH,
};
static MOVES_DISEASE_BITE: MoveTable = MoveTable {
    states: STATES,
    initial: DISEASE_BITE,
};
static MOVES_SCREECH: MoveTable = MoveTable {
    states: STATES,
    initial: SCREECH,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TwoTailedRat {
    /// `StarterMoveIndex`, set by the encounter; -1 opens with the random branch.
    pub starter_move_index: i32,
    /// `TurnsUntilSummonable`.
    pub turns_until_summonable: i32,
    /// `CallForBackupCount`.
    pub call_for_backup_count: i32,
}

impl Default for TwoTailedRat {
    /// A rat as `ModelDb.Monster<TwoTailedRat>().ToMutable()` makes it.
    fn default() -> Self {
        Self {
            starter_move_index: -1,
            turns_until_summonable: 2,
            call_for_backup_count: 0,
        }
    }
}

impl TwoTailedRat {
    /// A rat with `StarterMoveIndex` set.
    #[must_use]
    pub fn with_starter(starter_move_index: i32) -> Self {
        Self {
            starter_move_index,
            ..Self::default()
        }
    }
}

fn fields(s: &CombatState, id: CreatureId) -> TwoTailedRat {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::TwoTailedRat(r)) => r,
        _ => unreachable!("the owner is a TwoTailedRat"),
    }
}

fn set_fields(eng: &mut Engine<'_>, id: CreatureId, f: impl FnOnce(&mut TwoTailedRat)) {
    update_kind(eng, id, |kind| match kind {
        MonsterKind::TwoTailedRat(r) => f(r),
        _ => unreachable!("the owner is a TwoTailedRat"),
    });
}

/// `CanSummon`.
fn can_summon(s: &CombatState, me: CreatureId) -> bool {
    let rat = fields(s, me);
    if rat.turns_until_summonable > 0
        || rat.call_for_backup_count >= CALL_FOR_BACKUP_LIMIT
        || s.encounter.next_slot(s).is_none()
    {
        return false;
    }
    // `GetTeammatesOf(Creature)` without the creature itself.
    !s.enemies.iter().filter(|&&e| e != me).any(|&e| {
        s.creature(e)
            .monster
            .as_ref()
            .is_some_and(|m| m.next_move_name() == CALL_FOR_BACKUP_MOVE)
    })
}

#[sts2_macros::hooks]
impl Hooks for TwoTailedRat {
    type Me = CreatureId;
}

impl MonsterModel for TwoTailedRat {
    fn name(&self) -> &'static str {
        "Two-Tailed Rat"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 18, 17)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 22, 21)
    }

    fn moves(&self) -> &'static MoveTable {
        if self.starter_move_index == -1 {
            return &MOVES_RAND;
        }
        match self.starter_move_index % 3 {
            0 => &MOVES_SCRATCH,
            1 => &MOVES_DISEASE_BITE,
            _ => &MOVES_SCREECH,
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        if state != CALL_FOR_BACKUP {
            set_fields(eng, me, |r| r.turns_until_summonable -= 1);
        }
        match state {
            SCRATCH => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8)),
            DISEASE_BITE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6)),
            SCREECH => {
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::ONE, Some(me))
            }
            CALL_FOR_BACKUP => {
                // `Slots.LastOrDefault(s => Enemies.All(c => c.SlotName != s))`.
                let free = eng.s.encounter.slots().iter().rev().copied().find(|&slot| {
                    eng.s
                        .enemies
                        .iter()
                        .all(|&e| eng.s.creature(e).slot != Some(slot))
                });
                if free.is_some() {
                    let rat = MonsterKind::TwoTailedRat(Self::default());
                    eng.add_enemy(rat, free)?;
                }
                let rats: Vec<CreatureId> = eng
                    .s
                    .enemies
                    .iter()
                    .copied()
                    .filter(|&e| {
                        matches!(
                            eng.s.creature(e).monster.as_ref().map(|m| m.kind),
                            Some(MonsterKind::TwoTailedRat(_))
                        )
                    })
                    .collect();
                let count = rats
                    .iter()
                    .map(|&r| fields(eng.s, r).call_for_backup_count + 1)
                    .max()
                    .expect("the calling rat is in combat");
                for rat in rats {
                    set_fields(eng, rat, |r| r.call_for_backup_count = count);
                }
                Ok(())
            }
            _ => unreachable!("not a TwoTailedRat move: {state}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ONE_TWELFTH;

    #[test]
    fn one_twelfth_is_the_single_precision_quotient() {
        assert_eq!(ONE_TWELFTH.to_bits(), (1.0_f32 / 12.0_f32).to_bits());
    }
}
