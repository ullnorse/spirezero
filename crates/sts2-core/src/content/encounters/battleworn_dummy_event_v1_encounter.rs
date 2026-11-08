//! `BattlewornDummyEventV1Encounter` (`Models.Encounters/BattlewornDummyEventV1Encounter.cs`):
//! one `BattleFriendV1`, a monster room without rewards (`ShouldGiveRewards` is false).
//! `BattlewornDummy` starts it; `RanOutOfTime` (`BattlewornDummyEventEncounter`) is
//! `EncounterData::BattlewornDummy`.

use super::EncounterModel;
use crate::content::monsters::{BattleFriendV1, MonsterKind};
use crate::rng::Rng;

/// `BattlewornDummyEventV1Encounter`.
pub struct BattlewornDummyEventV1Encounter;

impl EncounterModel for BattlewornDummyEventV1Encounter {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::BattleFriendV1(BattleFriendV1)]
    }

    fn should_give_rewards(&self) -> bool {
        false
    }
}
