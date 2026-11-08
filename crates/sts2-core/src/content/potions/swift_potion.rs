//! `SwiftPotion` (`Models.Potions/SwiftPotion.cs`): the target player draws 3 cards.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `CardsVar(3)`.
const CARDS: i32 = 3;

pub struct SwiftPotion;

#[sts2_macros::hooks]
impl Hooks for SwiftPotion {
    type Me = PotionId;
}

impl PotionModel for SwiftPotion {
    fn name(&self) -> &'static str {
        "Swift Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // CardPileCmd.Draw(ctx, Cards.BaseValue, target.Player): the only player.
        eng.draw(Decimal::from(CARDS), false)?;
        Ok(())
    }
}
