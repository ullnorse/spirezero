//! `RubyRaidersNormal` (`Models.Encounters/RubyRaidersNormal.cs`): three different raiders,
//! each a `NextItem` on the encounter's own `Rng` over the raiders not picked yet, in the
//! dictionary's insertion order (Axe, Assassin, Brute, Crossbow, Tracker; one of each allowed).

use super::EncounterModel;
use crate::content::monsters::{
    AssassinRubyRaider, AxeRubyRaider, BruteRubyRaider, CrossbowRubyRaider, MonsterKind,
    TrackerRubyRaider,
};
use crate::rng::Rng;

/// `RubyRaidersNormal`: three different Ruby Raiders.
pub struct RubyRaidersNormal;

/// `_raiderValidCounts.Keys`, each with a valid count of 1.
const RAIDERS: [MonsterKind; 5] = [
    MonsterKind::AxeRubyRaider(AxeRubyRaider),
    MonsterKind::AssassinRubyRaider(AssassinRubyRaider),
    MonsterKind::BruteRubyRaider(BruteRubyRaider),
    MonsterKind::CrossbowRubyRaider(CrossbowRubyRaider),
    MonsterKind::TrackerRubyRaider(TrackerRubyRaider),
];

impl EncounterModel for RubyRaidersNormal {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let mut picked: Vec<MonsterKind> = Vec::new();
        for _ in 0..3 {
            let items: Vec<MonsterKind> = RAIDERS
                .into_iter()
                .filter(|raider| !picked.contains(raider))
                .collect();
            picked.push(*rng.next_item(&items).expect("raiders left"));
        }
        picked
    }
}
