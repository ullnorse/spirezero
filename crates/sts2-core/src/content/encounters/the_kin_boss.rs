//! `TheKinBoss` (`Models.Encounters/TheKinBoss.cs`): a dancing `KinFollower` in `slot1`, a
//! `KinFollower` in `slot2` and the `KinPriest` in `leaderSlot`.

use super::EncounterModel;
use crate::content::monsters::{KinFollower, KinPriest, MonsterKind};
use crate::rng::Rng;

/// `TheKinBoss`.
pub struct TheKinBoss;

impl EncounterModel for TheKinBoss {
    fn slots(&self) -> &'static [&'static str] {
        &["slot1", "slot2", "leaderSlot"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["slot1", "slot2", "leaderSlot"]
    }

    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::KinFollower(KinFollower {
                starts_with_dance: true,
            }),
            MonsterKind::KinFollower(KinFollower::default()),
            MonsterKind::KinPriest(KinPriest),
        ]
    }
}
