//! `SlitheringStranglerNormal` (`Models.Encounters/SlitheringStranglerNormal.cs`): a secondary
//! enemy picked by `NextItem` over `[SnappingJaxfruit, MediumSlime, SmallSlimes]` (a medium slime
//! picked from `[LeafSlimeM, TwigSlimeM]`, or two small slimes each picked from
//! `[LeafSlimeS, TwigSlimeS]` with replacement), then a `SlitheringStrangler`.

use super::EncounterModel;
use crate::content::monsters::{
    LeafSlimeM, LeafSlimeS, MonsterKind, SlitheringStrangler, SnappingJaxfruit, TwigSlimeM,
    TwigSlimeS,
};
use crate::rng::Rng;

/// `SlitheringStranglerNormal`: a secondary enemy, then a `SlitheringStrangler`.
pub struct SlitheringStranglerNormal;

/// `SecondaryEnemyType`, in declaration order.
#[derive(Clone, Copy)]
enum SecondaryEnemyType {
    SnappingJaxfruit,
    MediumSlime,
    SmallSlimes,
}

impl EncounterModel for SlitheringStranglerNormal {
    fn generate_monsters(&self, rng: &mut Rng) -> Vec<MonsterKind> {
        let small_slimes = [
            MonsterKind::LeafSlimeS(LeafSlimeS),
            MonsterKind::TwigSlimeS(TwigSlimeS),
        ];
        let medium_slimes = [
            MonsterKind::LeafSlimeM(LeafSlimeM),
            MonsterKind::TwigSlimeM(TwigSlimeM),
        ];
        let secondary = [
            SecondaryEnemyType::SnappingJaxfruit,
            SecondaryEnemyType::MediumSlime,
            SecondaryEnemyType::SmallSlimes,
        ];
        let mut monsters = match *rng.next_item(&secondary).expect("three secondary types") {
            SecondaryEnemyType::SnappingJaxfruit => {
                vec![MonsterKind::SnappingJaxfruit(SnappingJaxfruit)]
            }
            SecondaryEnemyType::MediumSlime => {
                vec![*rng.next_item(&medium_slimes).expect("two medium slimes")]
            }
            SecondaryEnemyType::SmallSlimes => {
                let first = *rng.next_item(&small_slimes).expect("two small slimes");
                let second = *rng.next_item(&small_slimes).expect("two small slimes");
                vec![first, second]
            }
        };
        monsters.push(MonsterKind::SlitheringStrangler(SlitheringStrangler));
        monsters
    }
}
