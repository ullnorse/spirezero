//! `Pael` (`Models.Events/Pael.cs`): one of Hive's ancients. Three relic options, one per pool
//! (`GenerateInitialOptions`, `Pael.cs:160-181`): `NextItem` of pool 1; `NextItem` of pool 2
//! with `PaelsClaw` (3+ deck cards `Goopy` can enchant) and `PaelsTooth` (5+ removable deck
//! cards) added, the list doubled (`AddRange(list)` of itself), then `PaelsGrowth` appended;
//! `NextItem` of pool 3 plus `PaelsLegion` when the player has no event pet.

use crate::content::ancients::{AncientModel, AncientOption};
use crate::content::enchantments::EnchantmentKind;
use crate::content::events::byrdonis_nest::has_event_pet;
use crate::rng::Rng;
use crate::run::RunState;

/// Hive's ancient (`Hive.GetUnlockedAncients`).
pub struct Pael;

/// `OptionPool1`.
const POOL_1: [&str; 3] = ["PAELS_FLESH", "PAELS_HORN", "PAELS_TEARS"];
/// `OptionPool2`.
const POOL_2: [&str; 1] = ["PAELS_WING"];
/// `OptionPool3`.
const POOL_3: [&str; 2] = ["PAELS_EYE", "PAELS_BLOOD"];

/// `Goopy.CanEnchant(c)` count that adds `PaelsClaw`.
const CLAW_CARDS: usize = 3;
/// `IsRemovable` count that adds `PaelsTooth`.
const TOOTH_CARDS: usize = 5;

impl AncientModel for Pael {
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let first = *rng.next_item(&POOL_1).expect("a pool");
        let mut second = POOL_2.to_vec();
        // A deck card the run does not port cannot take `Goopy` (a Defend tag would be ported).
        let goopy = s
            .deck
            .iter()
            .filter(|c| matches!(c.can_enchant(EnchantmentKind::Goopy), Ok(true)))
            .count();
        if goopy >= CLAW_CARDS {
            second.push("PAELS_CLAW");
        }
        if s.deck.iter().filter(|c| c.is_removable()).count() >= TOOTH_CARDS {
            second.push("PAELS_TOOTH");
        }
        second.extend_from_within(..);
        second.push("PAELS_GROWTH");
        let second = *rng.next_item(&second).expect("a pool");
        let mut third = POOL_3.to_vec();
        if !has_event_pet(s) {
            third.push("PAELS_LEGION");
        }
        let third = *rng.next_item(&third).expect("a pool");
        [first, second, third]
            .into_iter()
            .map(AncientOption::relic)
            .collect()
    }
}
