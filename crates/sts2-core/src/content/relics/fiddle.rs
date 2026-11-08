//! `Fiddle` (`Models.Relics/Fiddle.cs`): Ancient (Vakuu); draw 2 more cards for the hand draw,
//! and no other card is drawn during the owner's own turn (`ShouldDraw`).

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardsVar(2)`.
const CARDS: Decimal = Decimal::new(2, 0);

pub struct Fiddle;

impl RelicModel for Fiddle {
    fn name(&self) -> &'static str {
        "Fiddle"
    }
}

#[sts2_macros::hooks]
impl Hooks for Fiddle {
    type Me = RelicId;

    fn modify_hand_draw(&self, _s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        count + CARDS
    }

    /// Vetoes a draw that is not the hand draw while it is the player side's turn
    /// (`Creature.Side == CurrentSide`); `AfterPreventingDraw` only flashes.
    fn should_draw(&self, s: &CombatState, _me: RelicId, from_hand_draw: bool) -> bool {
        from_hand_draw || s.side != Side::Player
    }
}
