//! `Bellows` (`Models.Relics/Bellows.cs`): upgrade every card in the hand on the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::relics::RelicModel;

pub struct Bellows;

impl RelicModel for Bellows {
    fn name(&self) -> &'static str {
        "Bellows"
    }
}

#[sts2_macros::hooks]
impl Hooks for Bellows {
    type Me = RelicId;

    fn after_player_turn_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number > 1 {
            return Ok(());
        }
        // CardCmd.Upgrade(Hand.Cards, ..): in hand order.
        for card in eng.s.player.hand.clone() {
            eng.upgrade_card(card);
        }
        Ok(())
    }
}
