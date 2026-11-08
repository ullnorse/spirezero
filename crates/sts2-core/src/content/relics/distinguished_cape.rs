//! `DistinguishedCape` (`Models.Relics/DistinguishedCape.cs`): Ancient (Vakuu); `AfterObtained`
//! adds 2 random curses (`Rng.Niche`, as `NeowsBones`) and 3 `Apparition`s to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::{RelicModel, neows_bones};
use crate::run::{RunCard, RunCtx, RunError};

/// `DynamicVar("Curses", 2m)`.
const CURSES: usize = 2;
/// `CardsVar(3)`.
const CARDS: usize = 3;

pub struct DistinguishedCape;

impl RelicModel for DistinguishedCape {
    fn name(&self) -> &'static str {
        "Distinguished Cape"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        neows_bones::add_curses(s, CURSES);
        for _ in 0..CARDS {
            s.add_card(RunCard::new("APPARITION"));
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for DistinguishedCape {
    type Me = RelicId;
}
