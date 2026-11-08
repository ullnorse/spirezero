//! `Imbued` (`Models.Enchantments/Imbued.cs`): Skills only; the card starts at the bottom of the
//! draw pile and is auto-played when the owner's first turn enters its auto pre-play phase.
//! `ElectricShrymp` applies it.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, Engine, Flow};
use crate::content::cards::CardType;
use crate::content::enchantments::EnchantmentModel;

/// Auto-plays its card on the first turn.
pub struct Imbued;

impl EnchantmentModel for Imbued {
    fn name(&self) -> &'static str {
        "Imbued"
    }

    fn can_enchant_card_type(&self, card_type: CardType) -> bool {
        card_type == CardType::Skill
    }

    fn should_start_at_bottom_of_draw_pile(&self) -> bool {
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for Imbued {
    type Me = CardId;

    fn after_auto_pre_play_phase_entered(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        // `player == Card.Owner`: the only player owns every card.
        if eng.s.player.turn_number <= 1 {
            eng.auto_play(me, None)?;
        }
        Ok(())
    }
}
