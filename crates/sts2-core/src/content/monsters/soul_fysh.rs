//! `SoulFysh` (`Models.Monsters/SoulFysh.cs`): Beckon (a `Beckon` into the draw pile at a random
//! position and one into the discard pile) -> De-Gas -> Gaze (attack, a `Beckon` into the discard
//! pile) -> Fade (2 Intangible) -> Scream (attack, 3 Vulnerable) -> Beckon. `IsInvisible`, the music
//! hooks and `AfterCardChangedPilesLate` only drive visuals and audio.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType};
use crate::content::cards::CardKind;
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const BECKON: StateIdx = 0;
const DE_GAS: StateIdx = 1;
const GAZE: StateIdx = 2;
const SCREAM: StateIdx = 3;
const FADE: StateIdx = 4;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "BECKON_MOVE",
            follow_up: DE_GAS,
        },
        StateDef::Move {
            id: "DE_GAS_MOVE",
            follow_up: GAZE,
        },
        StateDef::Move {
            id: "GAZE_MOVE",
            follow_up: FADE,
        },
        StateDef::Move {
            id: "SCREAM_MOVE",
            follow_up: BECKON,
        },
        StateDef::Move {
            id: "FADE_MOVE",
            follow_up: SCREAM,
        },
    ],
    initial: BECKON,
};

/// `SoulFysh`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SoulFysh;

#[sts2_macros::hooks]
impl Hooks for SoulFysh {
    type Me = CreatureId;
}

impl MonsterModel for SoulFysh {
    fn name(&self) -> &'static str {
        "Soul Fysh"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 221, 211)
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
            BECKON => {
                // `CombatState.CreateCard<Beckon>(player)` per card, each added right after it.
                let draw = eng.create_card(CardKind::Beckon);
                eng.add_generated_card_to_combat_at(draw, PileType::Draw, None, Position::Random)?;
                let discard = eng.create_card(CardKind::Beckon);
                eng.add_generated_card_to_combat(discard, PileType::Discard, None)
            }
            DE_GAS => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 18, 16)),
            GAZE => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 8, 7))?;
                let card = eng.create_card(CardKind::Beckon);
                eng.add_generated_card_to_combat(card, PileType::Discard, None)
            }
            SCREAM => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 15, 13))?;
                let targets = eng.s.player_creatures();
                eng.apply_power_to_all(&targets, PowerKind::Vulnerable, Decimal::from(3), Some(me))
            }
            FADE => eng.apply_power(me, PowerKind::Intangible, Decimal::from(2), Some(me)),
            _ => unreachable!("not a SoulFysh move: {state}"),
        }
    }
}
