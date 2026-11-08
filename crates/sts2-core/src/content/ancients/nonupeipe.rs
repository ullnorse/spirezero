//! `Nonupeipe` (`Models.Events/Nonupeipe.cs`): one of Glory's ancients. `GenerateInitialOptions`
//! (`Nonupeipe.cs:140-149`) shuffles the nine-relic pool (`UnstableShuffle` on the event `Rng`),
//! adding `BeautifulBracelet` first when 4+ deck cards can take `Swift`, and offers the first 3.

use crate::content::ancients::{AncientModel, AncientOption};
use crate::content::enchantments::EnchantmentKind;
use crate::rng::Rng;
use crate::run::RunState;
use crate::shuffle::unstable_shuffle;

/// Glory's ancient (`Glory.GetUnlockedAncients`).
pub struct Nonupeipe;

/// `OptionPool`.
const POOL: [&str; 9] = [
    "BLESSED_ANTLER",
    "BRILLIANT_SCARF",
    "DELICATE_FROND",
    "DIAMOND_DIADEM",
    "FUR_COAT",
    "GLITTER",
    "JEWELRY_BOX",
    "LOOMING_FRUIT",
    "SIGNET_RING",
];

/// `Swift`-enchantable deck cards that add `BeautifulBracelet`.
const BRACELET_CARDS: usize = 4;

/// Options on the page (`list.Take(3)`).
const OPTIONS: usize = 3;

impl AncientModel for Nonupeipe {
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let mut list = POOL.to_vec();
        // A deck card the run does not port cannot take `Swift`.
        let swift = s
            .deck
            .iter()
            .filter(|c| matches!(c.can_enchant(EnchantmentKind::Swift), Ok(true)))
            .count();
        if swift >= BRACELET_CARDS {
            list.push("BEAUTIFUL_BRACELET");
        }
        unstable_shuffle(&mut list, rng);
        list.into_iter()
            .take(OPTIONS)
            .map(AncientOption::relic)
            .collect()
    }
}
