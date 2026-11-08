//! `LargeCapsule` (`Models.Relics/LargeCapsule.cs`): `AfterObtained` obtains 2 relics from the
//! front of the bag (`PullNextRelicFromFront`, then `RelicCmd.Obtain` each), then adds the
//! character's Basic Strike and Basic Defend to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

/// `IntVar("Relics", 2m)`.
const RELICS: usize = 2;

pub struct LargeCapsule;

impl RelicModel for LargeCapsule {
    fn name(&self) -> &'static str {
        "Large Capsule"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        for _ in 0..RELICS {
            let relic = ctx.pull_relic_from_front();
            ctx.obtain_relic(relic)?;
        }
        // `GetStrikeForCharacter`/`GetDefendForCharacter`: the first Basic card of the character
        // pool with the Strike, then the Defend, tag.
        let s = ctx.state();
        let first = |tagged: fn(RunCard) -> bool| {
            s.character
                .card_pool()
                .iter()
                .map(|c| RunCard::new(c.id))
                .find(|&c| tagged(c))
                .expect("every character pool has a Basic Strike and Defend")
        };
        let (strike, defend) = (first(RunCard::is_strike), first(RunCard::is_defend));
        s.add_card(strike);
        s.add_card(defend);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for LargeCapsule {
    type Me = RelicId;
}
