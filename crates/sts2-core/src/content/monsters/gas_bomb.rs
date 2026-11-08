//! `GasBomb` (`Models.Monsters/GasBomb.cs`): a Minion `LivingFog` summons. Its only move, Explode
//! (a `DeathBlowIntent`), attacks and then kills it. `HasExploded` only drives an animation.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, IntentType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const EXPLODE: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "EXPLODE_MOVE",
        follow_up: EXPLODE,
    }],
    initial: EXPLODE,
};

/// `GasBomb`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GasBomb;

#[sts2_macros::hooks]
impl Hooks for GasBomb {
    type Me = CreatureId;
}

impl MonsterModel for GasBomb {
    fn name(&self) -> &'static str {
        "Gas Bomb"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 8, 7)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn attack_intent_type(&self, _state: StateIdx) -> IntentType {
        IntentType::DeathBlow
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::Minion, Decimal::ONE, Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            EXPLODE => {
                attack_players(eng, me, if_ascension(eng.s.ascension, DEADLY_ENEMIES, 9, 8))?;
                eng.kill(&[me])
            }
            _ => unreachable!("not a GasBomb move: {state}"),
        }
    }
}
