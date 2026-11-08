//! `TouchOfInsanity` (`Models.Potions/TouchOfInsanity.cs`): choose a card in the hand that costs
//! energy or stars; it costs 0 for the rest of combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, Prompt, PromptText};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionKind, PotionModel, PotionUsage};

pub struct TouchOfInsanity;

#[sts2_macros::hooks]
impl Hooks for TouchOfInsanity {
    type Me = PotionId;
}

impl PotionModel for TouchOfInsanity {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Touch of Insanity"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // FromHand(.., c => c.CostsEnergyOrStars(false) || c.CostsEnergyOrStars(true)).
        let hooked = eng.hooked();
        let options: Vec<_> = hooked
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| {
                hooked.costs_energy_or_stars(c, false) || hooked.costs_energy_or_stars(c, true)
            })
            .collect();
        if let Some(&card) = eng
            .choose_cards(
                Prompt::potion(PotionKind::TouchOfInsanity, PromptText::Own),
                options,
                1,
                1,
            )?
            .first()
        {
            // SetToFreeThisCombat.
            eng.s.set_to_free_this_combat(card);
        }
        Ok(())
    }
}
