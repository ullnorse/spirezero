//! `DecimillipedeElite` (`Models.Encounters/DecimillipedeElite.cs`): the front, middle and back
//! `DecimillipedeSegment`s in `segment1` to `segment3`. The front's `StarterMoveIdx` is
//! `NextInt(3)` on the encounter's `Rng`, the others the next two (`% 3`).

use super::EncounterModel;
use crate::content::monsters::{
    DecimillipedeSegmentBack, DecimillipedeSegmentFront, DecimillipedeSegmentMiddle, MonsterKind,
};
use crate::rng::Rng;

/// `DecimillipedeElite`: three segments.
pub struct DecimillipedeElite;

impl EncounterModel for DecimillipedeElite {
    fn slots(&self) -> &'static [&'static str] {
        &["segment1", "segment2", "segment3"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["segment1", "segment2", "segment3"]
    }

    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let first = rng.next_int(3);
        vec![
            MonsterKind::DecimillipedeSegmentFront(DecimillipedeSegmentFront {
                starter_move_idx: first,
            }),
            MonsterKind::DecimillipedeSegmentMiddle(DecimillipedeSegmentMiddle {
                starter_move_idx: (first + 1) % 3,
            }),
            MonsterKind::DecimillipedeSegmentBack(DecimillipedeSegmentBack {
                starter_move_idx: (first + 2) % 3,
            }),
        ]
    }
}
