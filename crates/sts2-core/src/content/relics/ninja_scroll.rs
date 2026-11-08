//! `NinjaScroll` (`Models.Relics/NinjaScroll.cs`): before the owner's hand draw on turn 1 (or
//! earlier), creates 3 Shivs in the hand.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::cards::shiv;
use crate::content::relics::RelicModel;

/// `DynamicVar("Shivs", 3m)`.
const SHIVS: usize = 3;

pub struct NinjaScroll;

impl RelicModel for NinjaScroll {
    fn name(&self) -> &'static str {
        "Ninja Scroll"
    }
}

#[sts2_macros::hooks]
impl Hooks for NinjaScroll {
    type Me = RelicId;

    fn before_hand_draw(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number <= 1 {
            // Flash: visual only.
            shiv::create_in_hand(eng, SHIVS)?;
        }
        Ok(())
    }
}
