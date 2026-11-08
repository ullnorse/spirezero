//! `FlyconidNormal` (`Models.Encounters/FlyconidNormal.cs`): a medium slime picked from
//! `[LeafSlimeM, TwigSlimeM]` on the encounter's own `Rng`, then a `Flyconid`.

use super::EncounterModel;
use crate::content::monsters::{Flyconid, LeafSlimeM, MonsterKind, TwigSlimeM};
use crate::rng::Rng;

/// `FlyconidNormal`: a medium slime and a `Flyconid`.
pub struct FlyconidNormal;

impl EncounterModel for FlyconidNormal {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let medium_slimes = [
            MonsterKind::LeafSlimeM(LeafSlimeM),
            MonsterKind::TwigSlimeM(TwigSlimeM),
        ];
        let slime = *rng.next_item(&medium_slimes).expect("two medium slimes");
        vec![slime, MonsterKind::Flyconid(Flyconid)]
    }
}
