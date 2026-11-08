//! `DecimillipedeSegmentMiddle` (`Models.Monsters/DecimillipedeSegmentMiddle.cs`): a
//! `DecimillipedeSegment` (`decimillipede_segment.rs`, which holds all the logic); the subclass
//! only picks the visual scene.

use super::MonsterModel;
use super::decimillipede_segment as segment;
use super::machine::{MoveTable, StateIdx};
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DecimillipedeSegmentMiddle {
    /// `StarterMoveIdx`, set by the encounter.
    pub starter_move_idx: i32,
}

#[sts2_macros::hooks]
impl Hooks for DecimillipedeSegmentMiddle {
    type Me = CreatureId;
}

impl MonsterModel for DecimillipedeSegmentMiddle {
    fn name(&self) -> &'static str {
        segment::NAME
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        segment::min_initial_hp(ascension)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        segment::max_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        segment::moves(self.starter_move_idx)
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        segment::after_added_to_room(eng, me)
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        segment::perform_move(eng, me, state)
    }
}
