//! `FakeMerchantMonster` (`Models.Monsters/FakeMerchantMonster.cs`): Swipe first, then random
//! moves that never repeat: Swipe, Spew Coins (2×8), Throw Relic (an attack and 1 Frail, followed
//! by an attack only) or Enrage (2 Strength, 3-turn cooldown). The move dialogue draws
//! `Rng.Chaotic` (display only).

use super::machine::{Branch, MoveTable, Repeat, StateDef, StateIdx};
use super::{
    DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, attack_players_hits, if_ascension,
};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const SWIPE: StateIdx = 0;
const SPEW_COINS: StateIdx = 1;
const THROW_RELIC: StateIdx = 2;
const ENRAGE: StateIdx = 3;
const RAND: StateIdx = 4;
const RAND_ATTACK: StateIdx = 5;

/// `_spewCoinsDamage` and `_spewCoinsRepeat`.
const SPEW_COINS_DAMAGE: i32 = 2;
const SPEW_COINS_HITS: u32 = 8;
/// `EnrageMove`'s Strength.
const ENRAGE_STRENGTH: i32 = 2;
/// `ThrowRelicMove`'s Frail.
const THROW_RELIC_FRAIL: i32 = 1;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "SWIPE_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "SPEW_COINS_MOVE",
            follow_up: RAND,
        },
        StateDef::Move {
            id: "THROW_RELIC_MOVE",
            follow_up: RAND_ATTACK,
        },
        StateDef::Move {
            id: "ENRAGE_MOVE",
            follow_up: RAND,
        },
        StateDef::Random {
            id: "RAND_MOVE",
            branches: &[
                Branch::new(SWIPE, Repeat::CannotRepeat),
                Branch::new(SPEW_COINS, Repeat::CannotRepeat),
                Branch::new(THROW_RELIC, Repeat::CannotRepeat),
                // `AddBranch(moveState4, 3, MoveRepeatType.CannotRepeat)`: cooldown 3, weight 1.
                Branch {
                    state: ENRAGE,
                    repeat: Repeat::CannotRepeat,
                    cooldown: 3,
                    weight: 1.0,
                    weight_fn: None,
                },
            ],
        },
        StateDef::Random {
            id: "RAND_ATTACK_MOVE",
            branches: &[
                Branch::new(SWIPE, Repeat::CannotRepeat),
                Branch::new(SPEW_COINS, Repeat::CannotRepeat),
                Branch::new(THROW_RELIC, Repeat::CannotRepeat),
            ],
        },
    ],
    initial: SWIPE,
};

/// `FakeMerchantMonster`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FakeMerchantMonster;

#[sts2_macros::hooks]
impl Hooks for FakeMerchantMonster {
    type Me = CreatureId;
}

impl MonsterModel for FakeMerchantMonster {
    fn name(&self) -> &'static str {
        "The Merchant???"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 175, 165)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 175, 165)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            SWIPE => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 15, 13)),
            SPEW_COINS => attack_players_hits(eng, me, SPEW_COINS_DAMAGE, SPEW_COINS_HITS),
            THROW_RELIC => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 10, 9))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(
                    &targets,
                    PowerKind::Frail,
                    Decimal::from(THROW_RELIC_FRAIL),
                    Some(me),
                )
            }
            ENRAGE => eng.apply_power(
                me,
                PowerKind::Strength,
                Decimal::from(ENRAGE_STRENGTH),
                Some(me),
            ),
            _ => unreachable!("not a FakeMerchantMonster move: {state}"),
        }
    }
}
