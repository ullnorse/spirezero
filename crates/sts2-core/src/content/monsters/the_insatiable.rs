//! `TheInsatiable` (`Models.Monsters/TheInsatiable.cs`): opens with Liquify Ground (a 4
//! `SandpitPower` on itself aimed at each target, then six `FranticEscape`s per target's player,
//! three into the draw pile and three into the discard pile, each at a random position), then
//! Thrash (two hits) -> Lunging Bite (attack) -> Salivate (Strength) -> Thrash (`THRASH_MOVE_2`)
//! -> Thrash -> Lunging Bite... `HasLiquified`, `AfterDeath` and the animations are visual.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES};
use super::{attack_players, attack_players_hits, if_ascension};
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::{PowerKind, sandpit};
use crate::decimal::Decimal;

const LIQUIFY_GROUND: StateIdx = 0;
const LUNGING_BITE: StateIdx = 1;
const THRASH: StateIdx = 2;
const THRASH_2: StateIdx = 3;
const SALIVATE: StateIdx = 4;

/// `_thrashRepeat`.
const THRASH_REPEAT: u32 = 2;
/// Liquify's `SandpitPower` amount.
const SANDPIT: i32 = 4;
/// `FranticEscape`s per player; the first `_liquifyStatusDrawCount` (3) go to the draw pile.
const FRANTIC_ESCAPES: usize = 6;
/// `_liquifyStatusDrawCount`.
const STATUS_DRAW_COUNT: usize = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "LIQUIFY_GROUND_MOVE",
            follow_up: THRASH,
        },
        StateDef::Move {
            id: "LUNGING_BITE_MOVE",
            follow_up: SALIVATE,
        },
        StateDef::Move {
            id: "THRASH_MOVE",
            follow_up: LUNGING_BITE,
        },
        StateDef::Move {
            id: "THRASH_MOVE_2",
            follow_up: THRASH,
        },
        StateDef::Move {
            id: "SALIVATE_MOVE",
            follow_up: THRASH_2,
        },
    ],
    initial: LIQUIFY_GROUND,
};

/// `TheInsatiable`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TheInsatiable;

#[sts2_macros::hooks]
impl Hooks for TheInsatiable {
    type Me = CreatureId;
}

impl MonsterModel for TheInsatiable {
    fn name(&self) -> &'static str {
        "The Insatiable"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 341, 321)
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
            LIQUIFY_GROUND => {
                let targets = eng.s.player_creatures();
                for &target in &targets {
                    sandpit::apply(eng, me, target, Decimal::from(SANDPIT))?;
                }
                for _ in &targets {
                    // `CombatState.CreateCard<FranticEscape>(player)` per card, each added right
                    // after it.
                    for i in 0..FRANTIC_ESCAPES {
                        let pile = if i < STATUS_DRAW_COUNT {
                            PileType::Draw
                        } else {
                            PileType::Discard
                        };
                        let card = eng.create_card(CardKind::FranticEscape);
                        eng.add_generated_card_to_combat_at(card, pile, None, Position::Random)?;
                    }
                }
                Ok(())
            }
            THRASH | THRASH_2 => {
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 9, 8);
                attack_players_hits(eng, me, damage, THRASH_REPEAT)
            }
            LUNGING_BITE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 31, 28))
            }
            SALIVATE => {
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(amount), Some(me))
            }
            _ => unreachable!("not a TheInsatiable move: {state}"),
        }
    }
}
