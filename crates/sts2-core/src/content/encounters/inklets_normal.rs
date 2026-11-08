//! `InkletsNormal` (`Models.Encounters/InkletsNormal.cs`): three `Inklet`s; the middle one is
//! `MiddleInklet`.

use super::EncounterModel;
use crate::content::monsters::{Inklet, MonsterKind};
use crate::rng::Rng;

/// `InkletsNormal`: three `Inklet`s.
pub struct InkletsNormal;

impl EncounterModel for InkletsNormal {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        let outer = MonsterKind::Inklet(Inklet {
            middle_inklet: false,
        });
        let middle = MonsterKind::Inklet(Inklet {
            middle_inklet: true,
        });
        vec![outer, middle, outer]
    }
}
