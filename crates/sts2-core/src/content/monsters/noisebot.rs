//! `Noisebot` (`Models.Monsters/Noisebot.cs`): a `Fabricator` bot that makes Noise every turn: a
//! `Dazed` into the discard pile and one at a random position of the draw pile. The fall
//! position and the preview are cosmetic.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{MonsterModel, TOUGH_ENEMIES, if_ascension};
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PileType};
use crate::content::cards::CardKind;

const NOISE: StateIdx = 0;

static MOVES: MoveTable = MoveTable {
    states: &[StateDef::Move {
        id: "NOISE_MOVE",
        follow_up: NOISE,
    }],
    initial: NOISE,
};

/// `Noisebot`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Noisebot;

#[sts2_macros::hooks]
impl Hooks for Noisebot {
    type Me = CreatureId;
}

impl MonsterModel for Noisebot {
    fn name(&self) -> &'static str {
        "Noisebot"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 19, 18)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 24, 23)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, _me: CreatureId, state: StateIdx) -> Flow {
        match state {
            NOISE => {
                // One player: `target.Player ?? target.PetOwner` is always it.
                for _target in eng.s.player_creatures() {
                    let card = eng.create_card(CardKind::Dazed);
                    eng.add_generated_card_to_combat(card, PileType::Discard, None)?;
                    let card = eng.create_card(CardKind::Dazed);
                    eng.add_generated_card_to_combat_at(
                        card,
                        PileType::Draw,
                        None,
                        Position::Random,
                    )?;
                }
                Ok(())
            }
            _ => unreachable!("not a Noisebot move: {state}"),
        }
    }
}
