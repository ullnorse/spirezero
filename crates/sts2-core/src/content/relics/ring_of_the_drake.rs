//! `RingOfTheDrake` (`Models.Relics/RingOfTheDrake.cs`): Silent's upgraded starter relic
//! (`TouchOfOrobas`); draw 2 more cards on each of the first 3 turns.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardsVar(2)`.
const CARDS: i32 = 2;
/// `DynamicVar("Turns", 3m)`.
const TURNS: u32 = 3;

pub struct RingOfTheDrake;

impl RelicModel for RingOfTheDrake {
    fn name(&self) -> &'static str {
        "Ring of the Drake"
    }
}

#[sts2_macros::hooks]
impl Hooks for RingOfTheDrake {
    type Me = RelicId;

    fn modify_hand_draw(&self, s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        // The only player is the owner.
        if s.player.turn_number > TURNS {
            return count;
        }
        count + Decimal::from(CARDS)
    }
}
