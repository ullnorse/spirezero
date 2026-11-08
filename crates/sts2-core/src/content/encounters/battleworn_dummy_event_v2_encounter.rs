//! `BattlewornDummyEventV2Encounter` (`Models.Encounters/BattlewornDummyEventV2Encounter.cs`):
//! one `BattleFriendV2`, a monster room without rewards (`ShouldGiveRewards` is false).
//! `BattlewornDummy` starts it; `RanOutOfTime` (`BattlewornDummyEventEncounter`) is
//! `EncounterData::BattlewornDummy`.

use super::EncounterModel;
use crate::content::monsters::{BattleFriendV2, MonsterKind};
use crate::rng::Rng;

/// `BattlewornDummyEventV2Encounter`.
pub struct BattlewornDummyEventV2Encounter;

impl EncounterModel for BattlewornDummyEventV2Encounter {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::BattleFriendV2(BattleFriendV2)]
    }

    fn should_give_rewards(&self) -> bool {
        false
    }
}
