//! `BattlewornDummyEventV3Encounter` (`Models.Encounters/BattlewornDummyEventV3Encounter.cs`):
//! one `BattleFriendV3`, a monster room without rewards (`ShouldGiveRewards` is false).
//! `BattlewornDummy` starts it; `RanOutOfTime` (`BattlewornDummyEventEncounter`) is
//! `EncounterData::BattlewornDummy`.

use super::EncounterModel;
use crate::content::monsters::{BattleFriendV3, MonsterKind};
use crate::rng::Rng;

/// `BattlewornDummyEventV3Encounter`.
pub struct BattlewornDummyEventV3Encounter;

impl EncounterModel for BattlewornDummyEventV3Encounter {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::BattleFriendV3(BattleFriendV3)]
    }

    fn should_give_rewards(&self) -> bool {
        false
    }
}
