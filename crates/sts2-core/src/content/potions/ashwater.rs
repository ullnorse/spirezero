//! `Ashwater` (`Models.Potions/Ashwater.cs`): exhaust any number of cards of the hand (the
//! player's picks, zero to all).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, Prompt, PromptText};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionKind, PotionModel, PotionUsage};

pub struct Ashwater;

#[sts2_macros::hooks]
impl Hooks for Ashwater {
    type Me = PotionId;
}

impl PotionModel for Ashwater {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Ashwater"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        target.expect("AssertValidForTargetedPotion");
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, 0, 999999999), target.Player).
        let options = eng.s.player.hand.to_vec();
        let max = options.len();
        for card in eng.choose_cards(
            Prompt::potion(PotionKind::Ashwater, PromptText::Own),
            options,
            0,
            max,
        )? {
            eng.exhaust(card, false)?;
        }
        Ok(())
    }
}
