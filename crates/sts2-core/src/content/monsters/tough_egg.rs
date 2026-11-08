//! `ToughEgg` (`Models.Monsters/ToughEgg.cs`): an egg the `Ovicopter` lays. On joining it gains
//! `HatchPower` (1 on the player's turn, 2 on the enemies'); its first move hatches it: the
//! power and every power but `Minion` go, and its max and current HP are set to the hatchling's
//! (rolled on `Rng.Niche`). Then it repeats Nibble. The title (`HATCHLING.name` once hatched),
//! skins and `HatchPos` are visual; `IsHatched` is only ever set before joining by the hatch
//! itself, so the egg always joins unhatched.

use super::decimillipede_segment::set_max_and_current_hp;
use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, Side};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;
use crate::rng::RunRngType;

const HATCH: StateIdx = 0;
const NIBBLE: StateIdx = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "HATCH_MOVE",
            follow_up: NIBBLE,
        },
        StateDef::Move {
            id: "NIBBLE_MOVE",
            follow_up: NIBBLE,
        },
    ],
    initial: HATCH,
};

/// `ToughEgg`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ToughEgg;

#[sts2_macros::hooks]
impl Hooks for ToughEgg {
    type Me = CreatureId;
}

impl MonsterModel for ToughEgg {
    fn name(&self) -> &'static str {
        "Tough Egg"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 15, 14)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 19, 18)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = if eng.s.side == Side::Enemy { 2 } else { 1 };
        eng.apply_power(me, PowerKind::Hatch, Decimal::from(amount), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            HATCH => {
                let hatch = eng
                    .s
                    .creature(me)
                    .powers
                    .iter()
                    .find(|p| p.kind == PowerKind::Hatch)
                    .map(|p| p.id);
                if let Some(power) = hatch {
                    eng.remove_power(me, power)?;
                }
                let others: Vec<_> = eng
                    .s
                    .creature(me)
                    .powers
                    .iter()
                    .filter(|p| p.kind != PowerKind::Minion)
                    .map(|p| p.id)
                    .collect();
                for power in others {
                    eng.remove_power(me, power)?;
                }
                // `Hatch`: `ScaleHpForMultiplayer` is the identity for one player.
                let min = if_ascension(ascension, TOUGH_ENEMIES, 20, 19);
                let max = if_ascension(ascension, TOUGH_ENEMIES, 23, 22);
                let hp = eng.s.rng[RunRngType::Niche].next_int_range(min, max + 1);
                set_max_and_current_hp(eng, me, hp)
            }
            NIBBLE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 5, 4)),
            _ => unreachable!("not a ToughEgg move: {state}"),
        }
    }
}
