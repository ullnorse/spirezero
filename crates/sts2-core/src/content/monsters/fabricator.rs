//! `Fabricator` (`Models.Monsters/Fabricator.cs`): while fewer than 4 enemies are alive it picks
//! at random (equal weights) between Fabricate (a defensive bot, then an aggressive one) and
//! Fabricating Strike (attack, then an aggressive bot); otherwise it Disintegrates (attack).
//! Each bot is `MonsterAi.NextItem` over its set (`Guardbot`/`Noisebot` or `Zapbot`/`Stabbot`,
//! `HashSet` insertion order) without the last bot spawned, goes in `GetNextSlot` and gets
//! `Minion`.

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, Guardbot, MonsterKind, MonsterModel, Noisebot, Stabbot, TOUGH_ENEMIES, Zapbot,
    attack_players, if_ascension, update_kind,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;
use crate::rng::RunRngType;

const FABRICATE: StateIdx = 0;
const FABRICATING_STRIKE: StateIdx = 1;
const DISINTEGRATE: StateIdx = 2;
const FABRICATE_BRANCH: StateIdx = 3;
const RAND: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "FABRICATE_MOVE",
            follow_up: FABRICATE_BRANCH,
        },
        StateDef::Move {
            id: "FABRICATING_STRIKE_MOVE",
            follow_up: FABRICATE_BRANCH,
        },
        StateDef::Move {
            id: "DISINTEGRATE_MOVE",
            follow_up: FABRICATE_BRANCH,
        },
        StateDef::Conditional {
            id: "fabricateBranch",
            branches: &[(RAND, can_fabricate), (DISINTEGRATE, cannot_fabricate)],
        },
        StateDef::Random {
            id: "RAND",
            branches: &[
                Branch::weighted(FABRICATE, Repeat::CanRepeatForever, 1.0),
                Branch::weighted(FABRICATING_STRIKE, Repeat::CanRepeatForever, 1.0),
            ],
        },
    ],
    initial: FABRICATE_BRANCH,
};

/// A bot the Fabricator can spawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bot {
    Zapbot,
    Stabbot,
    Guardbot,
    Noisebot,
}

impl Bot {
    const fn monster(self) -> MonsterKind {
        match self {
            Self::Zapbot => MonsterKind::Zapbot(Zapbot),
            Self::Stabbot => MonsterKind::Stabbot(Stabbot),
            Self::Guardbot => MonsterKind::Guardbot(Guardbot),
            Self::Noisebot => MonsterKind::Noisebot(Noisebot),
        }
    }
}

/// `aggroSpawns`, in `HashSet` insertion order.
const AGGRO_SPAWNS: [Bot; 2] = [Bot::Zapbot, Bot::Stabbot];
/// `defenseSpawns`, in `HashSet` insertion order.
const DEFENSE_SPAWNS: [Bot; 2] = [Bot::Guardbot, Bot::Noisebot];

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Fabricator {
    /// `_lastSpawned`.
    pub last_spawned: Option<Bot>,
}

/// `CanFabricate`: fewer than 4 living enemies (the Fabricator included).
fn can_fabricate(s: &CombatState, _me: CreatureId) -> bool {
    s.enemies
        .iter()
        .filter(|&&e| s.creature(e).is_alive())
        .count()
        < 4
}

fn cannot_fabricate(s: &CombatState, me: CreatureId) -> bool {
    !can_fabricate(s, me)
}

/// `SpawnBot(options)`: `IsLiveCombat` is always true for a real combat.
fn spawn_bot(eng: &mut Engine<'_>, me: CreatureId, options: &[Bot]) -> Flow {
    let last = match eng.s.creature(me).monster.as_ref().map(|m| &m.kind) {
        Some(MonsterKind::Fabricator(f)) => f.last_spawned,
        _ => unreachable!("the owner is a Fabricator"),
    };
    let items: Vec<Bot> = options
        .iter()
        .copied()
        .filter(|&b| Some(b) != last)
        .collect();
    let bot = *eng.s.rng[RunRngType::MonsterAi]
        .next_item(&items)
        .expect("a bot set has two bots");
    update_kind(eng, me, |kind| match kind {
        MonsterKind::Fabricator(f) => f.last_spawned = Some(bot),
        _ => unreachable!("the owner is a Fabricator"),
    });
    let slot = eng.s.encounter.next_slot(eng.s);
    let target = eng.add_enemy(bot.monster(), slot)?;
    eng.apply_power(target, PowerKind::Minion, Decimal::ONE, Some(me))
}

#[sts2_macros::hooks]
impl Hooks for Fabricator {
    type Me = CreatureId;
}

impl MonsterModel for Fabricator {
    fn name(&self) -> &'static str {
        "Fabricator"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 155, 150)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            FABRICATE => {
                spawn_bot(eng, me, &DEFENSE_SPAWNS)?;
                spawn_bot(eng, me, &AGGRO_SPAWNS)
            }
            FABRICATING_STRIKE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 21, 18))?;
                spawn_bot(eng, me, &AGGRO_SPAWNS)
            }
            DISINTEGRATE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 13, 11))
            }
            _ => unreachable!("not a Fabricator move: {state}"),
        }
    }
}
