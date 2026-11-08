//! `SlimesNormal` (`Models.Encounters/SlimesNormal.cs`): `TwigSlimeM`, `LeafSlimeM`, then the two
//! small slimes in an order picked by one `NextBool` on the encounter's own `Rng`.

use super::EncounterModel;
use crate::content::monsters::{LeafSlimeM, LeafSlimeS, MonsterKind, TwigSlimeM, TwigSlimeS};
use crate::rng::Rng;

/// `SlimesNormal`: two medium and two small slimes.
pub struct SlimesNormal;

impl EncounterModel for SlimesNormal {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let leaf_first = rng.next_bool();
        let (first, second) = if leaf_first {
            (
                MonsterKind::LeafSlimeS(LeafSlimeS),
                MonsterKind::TwigSlimeS(TwigSlimeS),
            )
        } else {
            (
                MonsterKind::TwigSlimeS(TwigSlimeS),
                MonsterKind::LeafSlimeS(LeafSlimeS),
            )
        };
        vec![
            MonsterKind::TwigSlimeM(TwigSlimeM),
            MonsterKind::LeafSlimeM(LeafSlimeM),
            first,
            second,
        ]
    }
}
