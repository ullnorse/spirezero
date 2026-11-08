//! `Tezcatara` (`Models.Events/Tezcatara.cs`): one of Hive's ancients. Three relic options, one
//! per pool (`GenerateInitialOptions`, `Tezcatara.cs:188-200`), each a `NextItem` on the event
//! `Rng`: pool 1 gains `NutritiousSoup` when the deck has a Basic Strike-tagged card.

use crate::content::ancients::{AncientModel, AncientOption};
use crate::content::relics::nutritious_soup::is_basic_strike;
use crate::rng::Rng;
use crate::run::RunState;

/// Hive's ancient (`Hive.GetUnlockedAncients`).
pub struct Tezcatara;

/// `OptionPool1`.
const POOL_1: [&str; 2] = ["VERY_HOT_COCOA", "YUMMY_COOKIE"];
/// `OptionPool2`.
const POOL_2: [&str; 3] = ["BIIIG_HUG", "STORYBOOK", "TOASTY_MITTENS"];
/// `OptionPool3`.
const POOL_3: [&str; 4] = [
    "GOLDEN_COMPASS",
    "PUMPKIN_CANDLE",
    "TOY_BOX",
    "SEAL_OF_GOLD",
];

impl AncientModel for Tezcatara {
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let mut first = POOL_1.to_vec();
        if s.deck.iter().any(|&c| is_basic_strike(c)) {
            first.push("NUTRITIOUS_SOUP");
        }
        let first = *rng.next_item(&first).expect("a pool");
        let second = *rng.next_item(&POOL_2).expect("a pool");
        let third = *rng.next_item(&POOL_3).expect("a pool");
        [first, second, third]
            .into_iter()
            .map(AncientOption::relic)
            .collect()
    }
}
