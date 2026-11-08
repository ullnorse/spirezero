//! `GlowwaterPotion` (`Models.Potions/GlowwaterPotion.cs`): the target player exhausts their hand,
//! then draws 10 cards (`CardsVar(10)`).

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `CardsVar(10)`.
const CARDS: i32 = 10;

pub struct GlowwaterPotion;

#[sts2_macros::hooks]
impl Hooks for GlowwaterPotion {
    type Me = PotionId;
}

impl PotionModel for GlowwaterPotion {
    fn name(&self) -> &'static str {
        "Glowwater Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        // `AssertValidForTargetedPotion`; the only player owns the hand.
        target.expect("AssertValidForTargetedPotion");
        for card in eng.s.player.hand.clone() {
            eng.exhaust(card, false)?;
        }
        eng.draw(Decimal::from(CARDS), false)?;
        Ok(())
    }
}
