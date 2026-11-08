//! `DropletOfPrecognition` (`Models.Potions/DropletOfPrecognition.cs`): choose a card in the draw
//! pile and move it to the hand.

use crate::combat::PileType;
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, Prompt, PromptText};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionKind, PotionModel, PotionUsage};

pub struct DropletOfPrecognition;

#[sts2_macros::hooks]
impl Hooks for DropletOfPrecognition {
    type Me = PotionId;
}

impl PotionModel for DropletOfPrecognition {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Droplet of Precognition"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // CardSelectCmd.FromCombatPile(Draw, CardSelectorPrefs(prompt, 1)).FirstOrDefault().
        let options = eng.s.player.draw.to_vec();
        if let Some(&card) = eng
            .choose_cards(
                Prompt::potion(PotionKind::DropletOfPrecognition, PromptText::Own),
                options,
                1,
                1,
            )?
            .first()
        {
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
