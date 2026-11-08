//! `SlumberingBeetle` (`Models.Monsters/SlumberingBeetle.cs`): enters with `Plating` and 3
//! `Slumber`, snoring (`SNORE_MOVE`, nothing) while `Slumber` lasts, then Roll Out (attack, +2
//! Strength) forever. `SlumberPower` wakes it: by damage it is stunned into `WakeUpMove` ([`wake_up_stun`]), at
//! the end of the turn the move runs at once. `IsAwake` and the sleeping VFX are animation only.

use super::machine::{ForcedMove, MoveTable, StateDef, StateIdx, when};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SNORE: StateIdx = 0;
const SNORE_NEXT: StateIdx = 1;
const ROLL_OUT: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SNORE_MOVE",
            follow_up: SNORE_NEXT,
        },
        StateDef::Conditional {
            id: "SNORE_NEXT",
            branches: &[when(SNORE, is_slumbering), when(ROLL_OUT, is_awake)],
        },
        StateDef::Move {
            id: "ROLL_OUT_MOVE",
            follow_up: ROLL_OUT,
        },
    ],
    initial: SNORE,
};

fn is_slumbering(s: &CombatState, me: CreatureId) -> bool {
    s.creature(me).has_power(PowerKind::Slumber)
}

fn is_awake(s: &CombatState, me: CreatureId) -> bool {
    !is_slumbering(s, me)
}

/// `SlumberingBeetle`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlumberingBeetle;

impl SlumberingBeetle {
    /// `WakeUpMove`: the `Plating` goes (`IsAwake` and the animation are not state).
    pub(crate) fn wake_up_move(eng: &mut Engine<'_>, owner: CreatureId) -> Flow {
        let plating = eng
            .s
            .creature(owner)
            .powers
            .iter()
            .find(|p| p.kind == PowerKind::Plating)
            .map(|p| p.id);
        plating.map_or(Ok(()), |id| eng.remove_power(owner, id))
    }

    /// `CreatureCmd.Stun(owner, WakeUpMove, "ROLL_OUT_MOVE")`.
    pub(crate) fn wake_up_stun(eng: &mut Engine<'_>, owner: CreatureId) {
        super::stun_with(eng, owner, ForcedMove::BeetleWakeUp, Some(ROLL_OUT));
    }
}

#[sts2_macros::hooks]
impl Hooks for SlumberingBeetle {
    type Me = CreatureId;
}

impl MonsterModel for SlumberingBeetle {
    fn name(&self) -> &'static str {
        "Slumbering Beetle"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 89, 86)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let plating = if_ascension(eng.s.ascension, TOUGH_ENEMIES, 18, 15);
        eng.apply_power(me, PowerKind::Plating, Decimal::from(plating), Some(me))?;
        eng.apply_power(me, PowerKind::Slumber, Decimal::from(3), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            SNORE => Ok(()),
            ROLL_OUT => {
                let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 18, 16);
                attack_players(eng, me, damage)?;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not a SlumberingBeetle move: {state}"),
        }
    }
}
