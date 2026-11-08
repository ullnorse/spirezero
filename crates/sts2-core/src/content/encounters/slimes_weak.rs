//! `SlimesWeak` (`Models.Encounters/SlimesWeak.cs`): pick one small slime of
//! `[LeafSlimeS, TwigSlimeS]`, then the other one (still a `NextItem` draw over one item), then a
//! medium slime of `[LeafSlimeM, TwigSlimeM]`. Spawn order: first small, medium, second small.

use super::EncounterModel;
use crate::content::monsters::{LeafSlimeM, LeafSlimeS, MonsterKind, TwigSlimeM, TwigSlimeS};
use crate::rng::Rng;

/// `SlimesWeak`: small, medium, small slime, picked on the encounter's own RNG.
pub struct SlimesWeak;

impl EncounterModel for SlimesWeak {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let mut small = vec![
            MonsterKind::LeafSlimeS(LeafSlimeS),
            MonsterKind::TwigSlimeS(TwigSlimeS),
        ];
        let first = *rng.next_item(&small).expect("two small slimes");
        small.retain(|&m| m != first);
        let second = *rng.next_item(&small).expect("one small slime left");
        let medium = [
            MonsterKind::LeafSlimeM(LeafSlimeM),
            MonsterKind::TwigSlimeM(TwigSlimeM),
        ];
        let medium = *rng.next_item(&medium).expect("two medium slimes");
        vec![first, medium, second]
    }
}
