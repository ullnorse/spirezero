//! `GamblersBrew` (`Models.Potions/GamblersBrew.cs`): discard any number of cards from the hand,
//! then draw that many.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, Prompt, PromptText};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionKind, PotionModel, PotionUsage};

pub struct GamblersBrew;

#[sts2_macros::hooks]
impl Hooks for GamblersBrew {
    type Me = PotionId;
}

impl PotionModel for GamblersBrew {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Gambler's Brew"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // FromHandForDiscard(.., CardSelectorPrefs(prompt, 0, 999999999), null): at most the hand.
        let options = eng.s.player.hand.to_vec();
        let max = options.len();
        let cards = eng.choose_cards(
            Prompt::potion(PotionKind::GamblersBrew, PromptText::Own),
            options,
            0,
            max,
        )?;
        let draw = i32::try_from(cards.len()).expect("hand size fits i32");
        eng.discard_and_draw(&cards, draw)
    }
}
