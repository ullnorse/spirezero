//! `Orobas` (`Models.Events/Orobas.cs`): the Hive's ancient. On the event `Rng`: a character
//! other than the player's (`NextItem` over `UnlockState.Characters`), then `NextFloat` for
//! `PrismaticGem` (1/3) or a `SeaGlass` of that character joining pool 1, then one `NextItem`
//! each of pool 1, pool 2 and pool 3 (`TouchOfOrobas` and `ArchaicTooth` when their
//! `SetupForPlayer` succeeds, else a locked option).

use crate::content::ancients::{AncientModel, AncientOption};
use crate::content::relics::{archaic_tooth, sea_glass, touch_of_orobas};
use crate::rng::Rng;
use crate::run::RunState;
use crate::run::odds::roll_orobas_prismatic;

/// `OptionPool1`, in declaration order.
const OPTION_POOL_1: [&str; 2] = ["ELECTRIC_SHRYMP", "GLASS_EYE"];
/// `OptionPool2`, in declaration order.
const OPTION_POOL_2: [&str; 4] = [
    "ALCHEMICAL_COFFER",
    "DRIFTWOOD",
    "RADIANT_PEARL",
    "SAND_CASTLE",
];
/// `OROBAS.pages.INITIAL.options.OPTION_POOL_3_LOCKED`.
const OPTION_POOL_3_LOCKED: &str = "OPTION_POOL_3_LOCKED";

/// The Hive's ancient.
pub struct Orobas;

/// `OptionPool3` (`Orobas.cs:59-94`) for the run as entered.
fn option_pool_3(s: &RunState) -> Vec<AncientOption> {
    let mut list = Vec::new();
    if let Some(data) = touch_of_orobas::setup_for_player(s) {
        list.push(AncientOption::Relic {
            id: "TOUCH_OF_OROBAS",
            data: Some(data),
        });
    }
    if let Some(data) = archaic_tooth::setup_for_player(s) {
        list.push(AncientOption::Relic {
            id: "ARCHAIC_TOOTH",
            data: Some(data),
        });
    }
    if list.is_empty() {
        list.push(AncientOption::Locked(OPTION_POOL_3_LOCKED));
    }
    list
}

impl AncientModel for Orobas {
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let character = s.character;
        let others: Vec<_> = s
            .unlock
            .characters()
            .into_iter()
            .filter(|&c| c != character)
            .collect();
        let other = rng.next_item(&others).copied().unwrap_or(character);
        let mut pool_1: Vec<AncientOption> = OPTION_POOL_1
            .into_iter()
            .map(AncientOption::relic)
            .collect();
        pool_1.push(if roll_orobas_prismatic(rng) {
            AncientOption::relic("PRISMATIC_GEM")
        } else {
            AncientOption::Relic {
                id: "SEA_GLASS",
                data: Some(sea_glass::for_character(other)),
            }
        });
        let first = *rng.next_item(&pool_1).expect("pool 1 has options");
        let second = AncientOption::relic(
            rng.next_item(&OPTION_POOL_2)
                .copied()
                .expect("pool 2 has options"),
        );
        let third = *rng
            .next_item(&option_pool_3(s))
            .expect("pool 3 has options");
        vec![first, second, third]
    }
}
