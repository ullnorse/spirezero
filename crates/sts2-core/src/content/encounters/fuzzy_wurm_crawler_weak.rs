//! `FuzzyWurmCrawlerWeak` (`Models.Encounters/FuzzyWurmCrawlerWeak.cs`): one `FuzzyWurmCrawler`.

use super::EncounterModel;
use crate::content::monsters::{FuzzyWurmCrawler, MonsterKind};
use crate::rng::Rng;

/// `FuzzyWurmCrawlerWeak`: one `FuzzyWurmCrawler`.
pub struct FuzzyWurmCrawlerWeak;

impl EncounterModel for FuzzyWurmCrawlerWeak {
    fn generate_monsters(&self, _rng: &mut Rng) -> Vec<MonsterKind> {
        vec![MonsterKind::FuzzyWurmCrawler(FuzzyWurmCrawler::new())]
    }
}
