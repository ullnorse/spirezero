//! `OvergrowthCrawlers` (`Models.Encounters/OvergrowthCrawlers.cs`): a `ShrinkerBeetle`, then a
//! `FuzzyWurmCrawler`.

use super::EncounterModel;
use crate::content::monsters::{FuzzyWurmCrawler, MonsterKind, ShrinkerBeetle};
use crate::rng::Rng;

/// `OvergrowthCrawlers`: a `ShrinkerBeetle` and a `FuzzyWurmCrawler`.
pub struct OvergrowthCrawlers;

impl EncounterModel for OvergrowthCrawlers {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![
            MonsterKind::ShrinkerBeetle(ShrinkerBeetle),
            MonsterKind::FuzzyWurmCrawler(FuzzyWurmCrawler::new()),
        ]
    }
}
