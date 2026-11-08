//! `BowlbugsWeak` (`Models.Encounters/BowlbugsWeak.cs`): a `BowlbugRock` in the `odd` slot and an
//! `Egg` or `Nectar` (`NextItem` on the encounter's `Rng`) in the `even` slot. The encounter has no
//! `Slots`, so the enemies stay in spawn order.

use super::EncounterModel;
use crate::content::monsters::{BowlbugEgg, BowlbugNectar, BowlbugRock, MonsterKind};
use crate::rng::Rng;

/// `BowlbugsWeak`: a rock and an egg or a nectar bug.
pub struct BowlbugsWeak;

impl EncounterModel for BowlbugsWeak {
    fn spawn_slots(&self) -> &'static [&'static str] {
        &["odd", "even"]
    }

    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let bugs = [
            MonsterKind::BowlbugEgg(BowlbugEgg),
            MonsterKind::BowlbugNectar(BowlbugNectar),
        ];
        let bug = *rng.next_item(&bugs).expect("two bugs");
        vec![MonsterKind::BowlbugRock(BowlbugRock::default()), bug]
    }
}
