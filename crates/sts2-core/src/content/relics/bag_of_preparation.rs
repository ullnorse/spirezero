//! `BagOfPreparation` (`Models.Relics/BagOfPreparation.cs`): draw 2 more cards on the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardsVar(2)`.
const CARDS: i32 = 2;

pub struct BagOfPreparation;

impl RelicModel for BagOfPreparation {
    fn name(&self) -> &'static str {
        "Bag of Preparation"
    }
}

#[sts2_macros::hooks]
impl Hooks for BagOfPreparation {
    type Me = RelicId;

    fn modify_hand_draw(&self, s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        // The only player is the owner.
        if s.player.turn_number > 1 {
            return count;
        }
        count + Decimal::from(CARDS)
    }
}
