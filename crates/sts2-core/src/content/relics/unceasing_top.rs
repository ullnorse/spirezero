//! `UnceasingTop` (`Models.Relics/UnceasingTop.cs`): draws a card when the owner's hand empties
//! during the pre-play, play or post-play phase.

use crate::combat::TurnPhase;
use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

pub struct UnceasingTop;

impl RelicModel for UnceasingTop {
    fn name(&self) -> &'static str {
        "Unceasing Top"
    }
}

#[sts2_macros::hooks]
impl Hooks for UnceasingTop {
    type Me = RelicId;

    fn after_hand_emptied(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `IsValidPhase`: `(uint)(phase - 2) <= 2`, i.e. AutoPrePlay, Play or AutoPostPlay.
        if matches!(
            eng.s.player.phase,
            TurnPhase::AutoPrePlay | TurnPhase::Play | TurnPhase::AutoPostPlay
        ) {
            eng.draw(Decimal::ONE, false)?;
        }
        Ok(())
    }
}
