//! `DecimillipedeSegment` (`Models.Monsters/DecimillipedeSegment.cs`), the abstract base of the
//! three segments (`DecimillipedeSegmentFront`, `Middle`, `Back`, which only pick the visual
//! scene). A segment opens with Writhe, Bulk or Constrict by `StarterMoveIdx % 3` and cycles
//! Constrict -> Bulk -> Writhe -> Constrict. On joining it evens its max HP and makes it differ
//! from every teammate's, then gains `ReattachPower` (25). A dying segment is forced into
//! `DEAD_MOVE` (by the power), then `REATTACH_MOVE` heals it, then a random branch over the
//! three attacks, none twice in a row. The phobia-mode textures and segment shakes are visual.

use super::if_ascension;
use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterKind, TOUGH_ENEMIES, attack_players, attack_players_hits};
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::{PowerKind, reattach};
use crate::decimal::Decimal;

const WRITHE: StateIdx = 0;
const BULK: StateIdx = 1;
const CONSTRICT: StateIdx = 2;
const DEAD: StateIdx = 3;
const REATTACH: StateIdx = 4;
const RAND: StateIdx = 5;

/// `BulkStrength`.
const BULK_STRENGTH: i32 = 2;
/// The `ReattachPower` amount.
const REATTACH_AMOUNT: i32 = 25;

const STATES: &[StateDef] = &[
    StateDef::Move {
        id: "WRITHE_MOVE",
        follow_up: CONSTRICT,
    },
    StateDef::Move {
        id: "BULK_MOVE",
        follow_up: WRITHE,
    },
    StateDef::Move {
        id: "CONSTRICT_MOVE",
        follow_up: BULK,
    },
    StateDef::Move {
        id: "DEAD_MOVE",
        follow_up: REATTACH,
    },
    // `MustPerformOnceBeforeTransitioning`: it is only entered by a roll from `DEAD_MOVE` and
    // performed on the same round, so the machine never asks before it was performed.
    StateDef::Move {
        id: "REATTACH_MOVE",
        follow_up: RAND,
    },
    StateDef::Random {
        id: "RAND",
        branches: &[
            Branch::new(WRITHE, Repeat::CannotRepeat),
            Branch::new(BULK, Repeat::CannotRepeat),
            Branch::new(CONSTRICT, Repeat::CannotRepeat),
        ],
    },
];

static MOVES_WRITHE: MoveTable = MoveTable {
    states: STATES,
    initial: WRITHE,
};
static MOVES_BULK: MoveTable = MoveTable {
    states: STATES,
    initial: BULK,
};
static MOVES_CONSTRICT: MoveTable = MoveTable {
    states: STATES,
    initial: CONSTRICT,
};

/// The segment's English name (`DECIMILLIPEDE_SEGMENT.name`).
pub const NAME: &str = "Decimillipede";

/// `MinInitialHp`.
pub const fn min_initial_hp(ascension: u8) -> i32 {
    if_ascension(ascension, TOUGH_ENEMIES, 46, 40)
}

/// `MaxInitialHp`.
pub const fn max_initial_hp(ascension: u8) -> i32 {
    if_ascension(ascension, TOUGH_ENEMIES, 52, 46)
}

/// `GenerateMoveStateMachine`: the initial state is `StarterMoveIdx % 3`.
pub fn moves(starter_move_idx: i32) -> &'static MoveTable {
    match starter_move_idx % 3 {
        0 => &MOVES_WRITHE,
        1 => &MOVES_BULK,
        _ => &MOVES_CONSTRICT,
    }
}

/// Whether `kind` is a `DecimillipedeSegment`.
pub const fn is_segment(kind: MonsterKind) -> bool {
    matches!(
        kind,
        MonsterKind::DecimillipedeSegmentFront(_)
            | MonsterKind::DecimillipedeSegmentMiddle(_)
            | MonsterKind::DecimillipedeSegmentBack(_)
    )
}

/// `Owner.Monster.SetMoveImmediate(segment.DeadState)` (`MonsterModel.cs:421-433`): if the next
/// move can transition away, `DEAD_MOVE` becomes the current state (`ForceCurrentState`, not
/// logged) and the next move.
pub fn set_dead_state(eng: &mut Engine<'_>, id: CreatureId) {
    if let Some(m) = eng.s.creature_mut(id).monster.as_mut()
        && m.machine.can_transition_away()
    {
        m.machine.current = DEAD;
        m.machine.forced = None;
        m.rolled = true;
    }
}

/// `CreatureCmd.SetMaxAndCurrentHp`: `SetMaxHp`, then `SetCurrentHp` (the HP hook only if it
/// changed, a kill at 0).
pub fn set_max_and_current_hp(eng: &mut Engine<'_>, id: CreatureId, amount: i32) -> Flow {
    eng.set_max_hp(id, Decimal::from(amount))?;
    let old = eng.s.creature(id).hp;
    let creature = eng.s.creature_mut(id);
    creature.hp = amount.min(creature.max_hp);
    let new = creature.hp;
    eng.s.recount_enemies();
    if new != old {
        eng.after_current_hp_changed(id, Decimal::from(new - old))?;
    }
    if eng.s.creature(id).is_dead() {
        eng.kill(&[id])?;
    }
    Ok(())
}

/// `AfterAddedToRoom`: max HP rounded up to even, then raised by 2 (wrapping from above
/// `MaxInitialHp` to `MinInitialHp`) while a teammate has the same max HP; then `ReattachPower`.
pub fn after_added_to_room(eng: &mut Engine<'_>, me: CreatureId) -> Flow {
    let ascension = eng.s.ascension;
    let mut max_hp = eng.s.creature(me).max_hp;
    if max_hp % 2 == 1 {
        max_hp += 1;
    }
    let side = eng.s.creature(me).side;
    let others: Vec<i32> = eng
        .s
        .creatures_on(side)
        .into_iter()
        .filter(|&c| c != me)
        .map(|c| eng.s.creature(c).max_hp)
        .collect();
    while others.contains(&max_hp) {
        max_hp += 2;
        if max_hp > max_initial_hp(ascension) {
            max_hp = min_initial_hp(ascension);
        }
    }
    set_max_and_current_hp(eng, me, max_hp)?;
    eng.apply_power(
        me,
        PowerKind::Reattach,
        Decimal::from(REATTACH_AMOUNT),
        Some(me),
    )
}

/// The segment's moves.
pub fn perform_move(eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
    let ascension = eng.s.ascension;
    match state {
        WRITHE => attack_players_hits(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 6, 5), 2),
        BULK => {
            attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 7, 6))?;
            eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(BULK_STRENGTH),
                Some(me),
            )
        }
        CONSTRICT => {
            attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8))?;
            let targets = eng.s.player_creatures();
            eng.apply_power_to_all(&targets, PowerKind::Weak, Decimal::ONE, Some(me))
        }
        DEAD => Ok(()),
        // `IsLiveCombat` is always true for a real combat.
        REATTACH => reattach::do_reattach(eng, me),
        _ => unreachable!("not a DecimillipedeSegment move: {state}"),
    }
}
