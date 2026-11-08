//! `CorpseSlug` (`Models.Monsters/CorpseSlug.cs`): Whip Slap -> Glomp -> Goop -> Whip Slap, starting
//! at the state `StarterMoveIdx % 3` picks (the encounter sets it so slugs open differently). It
//! enters with `Ravenous`. `IsRavenous` only drives an animation, so it is not kept.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;
use crate::rng::Rng;

const WHIP_SLAP: StateIdx = 0;
const GLOMP: StateIdx = 1;
const GOOP: StateIdx = 2;

const WHIP_SLAP_MOVE: StateDef = StateDef::Move {
    id: "WHIP_SLAP_MOVE",
    follow_up: GLOMP,
};
const GLOMP_MOVE: StateDef = StateDef::Move {
    id: "GLOMP_MOVE",
    follow_up: GOOP,
};
const GOOP_MOVE: StateDef = StateDef::Move {
    id: "GOOP_MOVE",
    follow_up: WHIP_SLAP,
};

const fn table(initial: StateIdx) -> MoveTable {
    MoveTable {
        states: &[WHIP_SLAP_MOVE, GLOMP_MOVE, GOOP_MOVE],
        initial,
    }
}

static MOVES_WHIP_SLAP: MoveTable = table(WHIP_SLAP);
static MOVES_GLOMP: MoveTable = table(GLOMP);
static MOVES_GOOP: MoveTable = table(GOOP);

/// Per-combat fields, set by the encounter before `SetUpForCombat`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CorpseSlug {
    /// `StarterMoveIdx`, in `0..3`.
    pub starter_move_idx: i32,
}

impl CorpseSlug {
    /// `EnsureCorpseSlugsStartWithDifferentMoves`: one `NextInt(3)` draw, then consecutive
    /// starter indices for the slugs, in order.
    pub(crate) fn with_different_moves(count: usize, rng: &mut Rng) -> Vec<Self> {
        let mut index = rng.next_int(3);
        (0..count)
            .map(|_| {
                let slug = Self {
                    starter_move_idx: index % 3,
                };
                index += 1;
                slug
            })
            .collect()
    }
}

#[sts2_macros::hooks]
impl Hooks for CorpseSlug {
    type Me = CreatureId;
}

impl MonsterModel for CorpseSlug {
    fn name(&self) -> &'static str {
        "Corpse Slug"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 27, 25)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 29, 27)
    }

    fn moves(&self) -> &'static MoveTable {
        match self.starter_move_idx % 3 {
            0 => &MOVES_WHIP_SLAP,
            1 => &MOVES_GLOMP,
            _ => &MOVES_GOOP,
        }
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            // `WhipSlapDamage` 3, `WhipSlapRepeat` 2.
            WHIP_SLAP => attack_players_hits(eng, me, 3, 2),
            GLOMP => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 9, 8)),
            GOOP => {
                // `GoopFrailAmt` is 2.
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Frail, Decimal::from(2), Some(me))
            }
            _ => unreachable!("not a CorpseSlug move: {state}"),
        }
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = if_ascension(eng.s.ascension, DEADLY_ENEMIES, 5, 4);
        eng.apply_power(me, PowerKind::Ravenous, Decimal::from(amount), Some(me))
    }
}
