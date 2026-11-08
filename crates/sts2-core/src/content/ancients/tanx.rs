//! `Tanx` (`Models.Events/Tanx.cs`): one of Glory's ancients. Three relics out of nine
//! (`GenerateInitialOptions`, `Tanx.cs:135-143`): `TriBoomerang` joins the pool when 3+ deck
//! cards can take `Instinct`, then `UnstableShuffle(Rng).Take(3)`.

use crate::content::ancients::{AncientModel, AncientOption};
use crate::content::enchantments::EnchantmentKind;
use crate::rng::Rng;
use crate::run::RunState;
use crate::shuffle::unstable_shuffle;

/// Glory's ancient (`Glory.GetUnlockedAncients`).
pub struct Tanx;

/// `BaseOptionPool`.
const BASE_POOL: [&str; 9] = [
    "CLAWS",
    "CROSSBOW",
    "IRON_CLUB",
    "MEAT_CLEAVER",
    "SAI",
    "SPIKED_GAUNTLETS",
    "TANXS_WHISTLE",
    "THROWING_AXE",
    "WAR_HAMMER",
];

/// `Instinct.CanEnchant(c)` count that adds `TriBoomerang`.
const BOOMERANG_CARDS: usize = 3;

impl AncientModel for Tanx {
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let mut pool = BASE_POOL.to_vec();
        let instinct = s
            .deck
            .iter()
            .filter(|c| matches!(c.can_enchant(EnchantmentKind::Instinct), Ok(true)))
            .count();
        if instinct >= BOOMERANG_CARDS {
            pool.push("TRI_BOOMERANG");
        }
        unstable_shuffle(&mut pool, rng);
        pool.into_iter().take(3).map(AncientOption::relic).collect()
    }
}
