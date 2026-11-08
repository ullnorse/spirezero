//! `FuzzyWurmCrawler` (`Models.Monsters/FuzzyWurmCrawler.cs`): Acid Goop, Inhale (+7 Strength),
//! Acid Goop, repeating. No random branches, so no `Rng.MonsterAi` draws.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

/// Strength gained by Inhale.
const INHALE_STRENGTH: i32 = 7;

const FIRST_ACID_GOOP: StateIdx = 0;
const INHALE: StateIdx = 1;
const ACID_GOOP: StateIdx = 2;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "FIRST_ACID_GOOP",
            follow_up: INHALE,
        },
        StateDef::Move {
            id: "INHALE",
            follow_up: ACID_GOOP,
        },
        StateDef::Move {
            id: "ACID_GOOP",
            follow_up: FIRST_ACID_GOOP,
        },
    ],
    initial: FIRST_ACID_GOOP,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FuzzyWurmCrawler {
    /// Animation-only.
    pub is_puffed: bool,
}

impl FuzzyWurmCrawler {
    /// The fields at `SetUpForCombat`.
    #[must_use]
    pub const fn new() -> Self {
        Self { is_puffed: false }
    }
}

#[sts2_macros::hooks]
impl Hooks for FuzzyWurmCrawler {
    type Me = CreatureId;
}

impl MonsterModel for FuzzyWurmCrawler {
    fn name(&self) -> &'static str {
        "Fuzzy Wurm Crawler"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 58, 55)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 59, 57)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        if state == INHALE {
            set_puffed(eng, me, true);
            eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(INHALE_STRENGTH),
                Some(me),
            )
        } else {
            set_puffed(eng, me, false);
            let damage = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 6, 4);
            attack_players(eng, me, damage)
        }
    }
}

fn set_puffed(eng: &mut Engine<'_>, id: CreatureId, puffed: bool) {
    super::update_kind(eng, id, |kind| {
        if let MonsterKind::FuzzyWurmCrawler(m) = kind {
            m.is_puffed = puffed;
        }
    });
}
