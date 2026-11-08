//! `LiquidMemories` (`Models.Potions/LiquidMemories.cs`): choose a card in the discard pile; it
//! returns to the hand and costs 0 this turn.

use crate::combat::PileType;
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, Prompt, PromptText};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionKind, PotionModel, PotionUsage};

pub struct LiquidMemories;

#[sts2_macros::hooks]
impl Hooks for LiquidMemories {
    type Me = PotionId;
}

impl PotionModel for LiquidMemories {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Liquid Memories"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // CardSelectCmd.FromCombatPile(Discard, CardSelectorPrefs(prompt, 1)).FirstOrDefault().
        let options = eng.s.player.discard.to_vec();
        if let Some(&card) = eng
            .choose_cards(
                Prompt::potion(PotionKind::LiquidMemories, PromptText::Own),
                options,
                1,
                1,
            )?
            .first()
        {
            // SetToFreeThisTurn.
            eng.s.set_to_free_this_turn(card);
            eng.add_to_pile(card, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}
