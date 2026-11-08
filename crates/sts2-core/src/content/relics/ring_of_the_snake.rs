//! `RingOfTheSnake` (`Models.Relics/RingOfTheSnake.cs`): Silent's starter relic; draw 2 more cards
//! on the first turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardsVar(2)`.
const CARDS: i32 = 2;

pub struct RingOfTheSnake;

impl RelicModel for RingOfTheSnake {
    fn name(&self) -> &'static str {
        "Ring of the Snake"
    }
}

#[sts2_macros::hooks]
impl Hooks for RingOfTheSnake {
    type Me = RelicId;

    fn modify_hand_draw(&self, s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        // The only player is the owner.
        if s.player.turn_number > 1 {
            return count;
        }
        count + Decimal::from(CARDS)
    }
}
