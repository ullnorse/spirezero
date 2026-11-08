//! `FrogKnight` (`Models.Monsters/FrogKnight.cs`): starts with Plating. Tongue Lash (damage,
//! 2 Frail) -> Strike Down Evil -> For the Queen (5 Strength) -> Tongue Lash, except that the
//! turn after For the Queen it plays Beetle Charge instead, once, when below half HP.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{
    DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const HALF_HEALTH: StateIdx = 0;
const FOR_THE_QUEEN: StateIdx = 1;
const STRIKE_DOWN_EVIL: StateIdx = 2;
const TONGUE_LASH: StateIdx = 3;
const BEETLE_CHARGE: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "HALF_HEALTH",
            branches: &[
                when(TONGUE_LASH, not_charging),
                when(BEETLE_CHARGE, charging),
            ],
        },
        StateDef::Move {
            id: "FOR_THE_QUEEN",
            follow_up: HALF_HEALTH,
        },
        StateDef::Move {
            id: "STRIKE_DOWN_EVIL",
            follow_up: FOR_THE_QUEEN,
        },
        StateDef::Move {
            id: "TONGUE_LASH",
            follow_up: STRIKE_DOWN_EVIL,
        },
        StateDef::Move {
            id: "BEETLE_CHARGE",
            follow_up: TONGUE_LASH,
        },
    ],
    initial: TONGUE_LASH,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FrogKnight {
    /// `_hasBeetleCharged`.
    pub has_beetle_charged: bool,
}

fn fields(s: &CombatState, id: CreatureId) -> FrogKnight {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::FrogKnight(m)) => m,
        _ => unreachable!("the owner is a FrogKnight"),
    }
}

/// `CurrentHp < MaxHp / 2` (integer division).
fn below_half(s: &CombatState, id: CreatureId) -> bool {
    let c = s.creature(id);
    c.hp < c.max_hp / 2
}

/// `HasBeetleCharged || CurrentHp >= MaxHp / 2`.
fn not_charging(s: &CombatState, id: CreatureId) -> bool {
    fields(s, id).has_beetle_charged || !below_half(s, id)
}

/// `!HasBeetleCharged && CurrentHp < MaxHp / 2`.
fn charging(s: &CombatState, id: CreatureId) -> bool {
    !fields(s, id).has_beetle_charged && below_half(s, id)
}

#[sts2_macros::hooks]
impl Hooks for FrogKnight {
    type Me = CreatureId;
}

impl MonsterModel for FrogKnight {
    fn name(&self) -> &'static str {
        "Frog Knight"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 199, 191)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let plating = if_ascension(eng.s.ascension, TOUGH_ENEMIES, 19, 15);
        eng.apply_power(me, PowerKind::Plating, Decimal::from(plating), Some(me))?;
        set_has_beetle_charged(eng, me, false);
        Ok(())
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            FOR_THE_QUEEN => eng.apply_power(me, PowerKind::Strength, Decimal::from(5), Some(me)),
            STRIKE_DOWN_EVIL => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 23, 21))
            }
            TONGUE_LASH => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 14, 13))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::from(2), Some(me))
            }
            BEETLE_CHARGE => {
                set_has_beetle_charged(eng, me, true);
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 40, 35))
            }
            _ => unreachable!("not a FrogKnight move: {state}"),
        }
    }
}

fn set_has_beetle_charged(eng: &mut Engine<'_>, id: CreatureId, value: bool) {
    super::update_kind(eng, id, |kind| {
        if let MonsterKind::FrogKnight(m) = kind {
            m.has_beetle_charged = value;
        }
    });
}
