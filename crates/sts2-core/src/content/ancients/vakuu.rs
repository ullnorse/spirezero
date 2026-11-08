//! `Vakuu` (`Models.Events/Vakuu.cs`): Glory's ancient. Three relic pools, each
//! `UnstableShuffle`d on the event `Rng` in order (pool 1, 2, 3); the options are the first
//! relic of each. `SereTalon.ThatDecreasesMaxHp(9m)` only makes the option flash red. The dialogue,
//! `CalculateVars` (`Visits`) and `VakuuCardSelector` (`Engine::vakuu_selector`) are not options.

use crate::content::ancients::{AncientModel, AncientOption};
use crate::rng::Rng;
use crate::run::RunState;
use crate::shuffle::unstable_shuffle;

/// `Pool1`.
const POOL_1: [&str; 3] = ["BLOOD_SOAKED_ROSE", "WHISPERING_EARRING", "FIDDLE"];
/// `Pool2`.
const POOL_2: [&str; 3] = ["PRESERVED_FOG", "SERE_TALON", "DISTINGUISHED_CAPE"];
/// `Pool3`.
const POOL_3: [&str; 4] = [
    "CHOICES_PARADOX",
    "MUSIC_BOX",
    "LORDS_PARASOL",
    "JEWELED_MASK",
];

/// Glory's ancient.
pub struct Vakuu;

impl AncientModel for Vakuu {
    fn generate_options(&self, _s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption> {
        let mut pool_1 = POOL_1.to_vec();
        let mut pool_2 = POOL_2.to_vec();
        let mut pool_3 = POOL_3.to_vec();
        unstable_shuffle(&mut pool_1, rng);
        unstable_shuffle(&mut pool_2, rng);
        unstable_shuffle(&mut pool_3, rng);
        [pool_1[0], pool_2[0], pool_3[0]]
            .into_iter()
            .map(AncientOption::relic)
            .collect()
    }
}
