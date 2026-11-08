//! `RazorTooth` (`Models.Relics/RazorTooth.cs`): an Attack or Skill the owner plays is upgraded
//! after it is played (`AfterCardPlayed`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, Engine, Flow, RelicId};
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;

pub struct RazorTooth;

impl RelicModel for RazorTooth {
    fn name(&self) -> &'static str {
        "Razor Tooth"
    }
}

#[sts2_macros::hooks]
impl Hooks for RazorTooth {
    type Me = RelicId;

    fn after_card_played(&self, eng: &mut Engine<'_>, _me: RelicId, play: &CardPlay) -> Flow {
        // `(uint)(type - 1) > 1u`: only Attack (1) and Skill (2).
        let card = eng.s.card(play.card);
        if matches!(card.card_type(), CardType::Attack | CardType::Skill) && card.is_upgradable() {
            eng.upgrade_card(play.card);
        }
        Ok(())
    }
}
