//! `FragrantMushroom` (`Models.Relics/FragrantMushroom.cs`): when obtained, 15 unblockable
//! unpowered damage, then 2 random upgradable deck cards upgrade (`StableShuffle` on
//! `Rng.Niche`). Event rarity (`HungryForMushrooms`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{RunCtx, RunError};
use crate::shuffle::stable_shuffle_by;

/// `HpLossVar(15m)`.
const HP_LOSS: i32 = 15;
/// `CardsVar(2)`.
const CARDS: usize = 2;

pub struct FragrantMushroom;

impl RelicModel for FragrantMushroom {
    fn name(&self) -> &'static str {
        "Fragrant Mushroom"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        s.damage(HP_LOSS);
        let deck = s.deck.clone();
        let mut cards: Vec<usize> = (0..deck.len())
            .filter(|&i| deck[i].is_upgradable())
            .collect();
        stable_shuffle_by(&mut cards, &mut s.rng[RunRngType::Niche], |&a, &b| {
            deck[a].compare_to(&deck[b])
        });
        for &i in cards.iter().take(CARDS) {
            s.deck[i].upgrade();
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for FragrantMushroom {
    type Me = RelicId;
}
