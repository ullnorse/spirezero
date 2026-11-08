//! `FakeMerchantEventEncounter` (`Models.Encounters/FakeMerchantEventEncounter.cs`): the
//! `FakeMerchantMonster` alone in the `merchant` slot, a monster room worth exactly 300 gold. Only
//! `FakeMerchant` starts it (thrown `FoulPotion`).

use super::EncounterModel;
use crate::content::monsters::{FakeMerchantMonster, MonsterKind};
use crate::rng::Rng;

/// `MinGoldReward` and `MaxGoldReward`.
const GOLD: i32 = 300;

/// `FakeMerchantEventEncounter`: `FakeMerchant`'s fight.
pub struct FakeMerchantEventEncounter;

impl EncounterModel for FakeMerchantEventEncounter {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::FakeMerchantMonster(FakeMerchantMonster)]
    }

    fn slots(&self) -> &'static [&'static str] {
        &["merchant"]
    }

    fn spawn_slots(&self) -> &'static [&'static str] {
        &["merchant"]
    }

    fn gold_reward(&self) -> Option<(i32, i32)> {
        Some((GOLD, GOLD))
    }
}
