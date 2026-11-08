//! `TheForgotten` (`Models.Monsters/TheForgotten.cs`): starts with Possess Speed. Miasma (steals 2
//! Dexterity from the player, 8 Block) -> Dread (damage plus its own Dexterity) -> Miasma.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, ValueProps};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const MIASMA: StateIdx = 0;
const DREAD: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "MIASMA",
            follow_up: DREAD,
        },
        StateDef::Move {
            id: "DREAD",
            follow_up: MIASMA,
        },
    ],
    initial: MIASMA,
};

/// `DebilitatingSmogDexStealAmount` (2 at every ascension).
const SMOG_DEXTERITY_STEAL: i32 = 2;
/// The Block Miasma gains.
const MIASMA_BLOCK: i32 = 8;

/// `TheForgotten`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TheForgotten;

#[sts2_macros::hooks]
impl Hooks for TheForgotten {
    type Me = CreatureId;
}

impl MonsterModel for TheForgotten {
    fn name(&self) -> &'static str {
        "The Forgotten"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 111, 106)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        eng.apply_power(me, PowerKind::PossessSpeed, Decimal::ONE, None)
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        match state {
            MIASMA => {
                let targets = eng.s.player_creatures();
                let steal = Decimal::from(SMOG_DEXTERITY_STEAL);
                eng.apply_power_to_all(
                    &targets,
                    PowerKind::Dexterity,
                    Decimal::ZERO - steal,
                    Some(me),
                )?;
                eng.gain_block(me, Decimal::from(MIASMA_BLOCK), ValueProps::MOVE, None)?;
                eng.apply_power(me, PowerKind::Dexterity, steal, Some(me))
            }
            DREAD => {
                // `DreadDamage`: read when the attack starts, after the Dexterity above.
                let base = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 15, 13);
                let dexterity = eng.s.creature(me).power_amount(PowerKind::Dexterity);
                attack_players(eng, me, base + dexterity)
            }
            _ => unreachable!("not a TheForgotten move: {state}"),
        }
    }
}
