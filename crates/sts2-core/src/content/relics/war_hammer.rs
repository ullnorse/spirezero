//! `WarHammer` (`Models.Relics/WarHammer.cs`): Ancient (Tanx); after an Elite fight is won, 4
//! upgradable deck cards are upgraded: the upgradable cards `StableShuffle`d on the run's `Niche`
//! stream (sorted by `CardModel.CompareTo`), the first 4.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{RoomType, RunCtx, RunError};
use crate::shuffle::stable_shuffle_by;

/// `CardsVar(4)`.
const CARDS: usize = 4;

pub struct WarHammer;

impl RelicModel for WarHammer {
    fn name(&self) -> &'static str {
        "War Hammer"
    }

    fn after_combat_victory_in_run(
        &self,
        ctx: &mut RunCtx<'_>,
        _me: usize,
        room: RoomType,
    ) -> Result<(), RunError> {
        if room != RoomType::Elite {
            return Ok(());
        }
        let s = ctx.state();
        let mut cards = s.upgradable_cards();
        let deck = s.deck.clone();
        let mut rng = s.rng[RunRngType::Niche].clone();
        stable_shuffle_by(&mut cards, &mut rng, |&a, &b| deck[a].compare_to(&deck[b]));
        s.rng[RunRngType::Niche] = rng;
        for &i in cards.iter().take(CARDS) {
            s.deck[i].upgrade();
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for WarHammer {
    type Me = RelicId;
}
