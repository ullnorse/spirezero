//! `BowlbugsNormal` (`Models.Encounters/BowlbugsNormal.cs`): a `BowlbugRock` in the `first` slot,
//! then two different workers (Egg, Silk, Nectar; each at most once, `NextItem` on the encounter's
//! `Rng` over the ones left, twice) in `middle` and `last`. The encounter has no `Slots`, so the
//! enemies stay in spawn order.

use super::EncounterModel;
use crate::content::monsters::{BowlbugEgg, BowlbugNectar, BowlbugRock, BowlbugSilk, MonsterKind};
use crate::rng::Rng;

/// `BowlbugsNormal`: a rock and two different workers.
pub struct BowlbugsNormal;

impl EncounterModel for BowlbugsNormal {
    fn spawn_slots(&self) -> &'static [&'static str] {
        &["first", "middle", "last"]
    }

    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        // `_workerValidCounts`, in insertion order.
        let mut workers = vec![
            MonsterKind::BowlbugEgg(BowlbugEgg),
            MonsterKind::BowlbugSilk(BowlbugSilk),
            MonsterKind::BowlbugNectar(BowlbugNectar),
        ];
        let mut monsters = vec![MonsterKind::BowlbugRock(BowlbugRock::default())];
        for _ in 0..2 {
            let worker = *rng.next_item(&workers).expect("workers left");
            workers.retain(|&w| w != worker);
            monsters.push(worker);
        }
        monsters
    }
}
