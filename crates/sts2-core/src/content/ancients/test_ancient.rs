//! Test-only ancient: shows that an ancient is its file. Like `Pael` (`Models.Events/Pael.cs`),
//! it rolls one option from each of three pools on the event `Rng`, reading the deck for one of
//! them, and the third pool holds a relic with preset fields and a locked option.

#![cfg(test)]

use crate::content::ancients::{AncientModel, AncientOption};
use crate::content::relics::{RelicData, lava_rock};
use crate::rng::Rng;
use crate::run::RunState;

/// Test-only: an ancient with three rolled options.
pub struct TestAncient;

impl AncientModel for TestAncient {
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let first = *rng
            .next_item(&["BOOMING_CONCH", "GOLDEN_PEARL"])
            .expect("a pool");
        let mut second = vec!["WINGED_BOOTS"];
        if s.deck.len() >= 10 {
            second.push("NEW_LEAF");
        }
        let second = *rng.next_item(&second).expect("a pool");
        let third = [
            AncientOption::Relic {
                id: "LAVA_ROCK",
                data: Some(RelicData::LavaRock(lava_rock::Data {
                    has_triggered: true,
                })),
            },
            AncientOption::Locked("POOL_3_LOCKED"),
        ];
        vec![
            AncientOption::relic(first),
            AncientOption::relic(second),
            *rng.next_item(&third).expect("a pool"),
        ]
    }
}
