//! `SlumberingBeetleNormal` (`Models.Encounters/SlumberingBeetleNormal.cs`): a `BowlbugRock`, a
//! `BowlbugSilk` and a `SlumberingBeetle`, in the slots `first`, `second` and `third`. The
//! encounter has no `Slots`, so the enemies stay in spawn order.

use super::EncounterModel;
use crate::content::monsters::{BowlbugRock, BowlbugSilk, MonsterKind, SlumberingBeetle};
use crate::rng::Rng;

/// `SlumberingBeetleNormal`: a rock, a silk and a sleeping beetle.
pub struct SlumberingBeetleNormal;

impl EncounterModel for SlumberingBeetleNormal {
    fn spawn_slots(&self) -> &'static [&'static str] {
        &["first", "second", "third"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::BowlbugRock(BowlbugRock::default()),
            MonsterKind::BowlbugSilk(BowlbugSilk),
            MonsterKind::SlumberingBeetle(SlumberingBeetle),
        ]
    }
}
