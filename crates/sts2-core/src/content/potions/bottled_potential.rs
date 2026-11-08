//! `BottledPotential` (`Models.Potions/BottledPotential.cs`): move the hand to the draw pile,
//! shuffle, then draw 5.

use crate::combat::PileType;
use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;

/// `CardsVar(5)`.
const CARDS: i32 = 5;

pub struct BottledPotential;

#[sts2_macros::hooks]
impl Hooks for BottledPotential {
    type Me = PotionId;
}

impl PotionModel for BottledPotential {
    fn name(&self) -> &'static str {
        "Bottled Potential"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::CombatOnly
    }

    fn target(&self) -> TargetType {
        TargetType::AnyPlayer
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let _ = target.expect("AssertValidForTargetedPotion");
        // CardPileCmd.Add(Hand.Cards, Draw): one batch, to the bottom.
        let hand = eng.s.player.hand.to_vec();
        eng.add_cards_to_pile(&hand, PileType::Draw, Position::Bottom)?;
        eng.shuffle()?;
        eng.draw(Decimal::from(CARDS), false)?;
        Ok(())
    }
}
