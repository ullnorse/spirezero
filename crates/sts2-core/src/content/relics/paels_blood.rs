//! `PaelsBlood` (`Models.Relics/PaelsBlood.cs`): Ancient (Pael); the owner's hand draw is 1 card
//! larger (`ModifyHandDraw`).

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `CardsVar(1)`.
const CARDS: Decimal = Decimal::ONE;

pub struct PaelsBlood;

impl RelicModel for PaelsBlood {
    fn name(&self) -> &'static str {
        "Pael's Blood"
    }
}

#[sts2_macros::hooks]
impl Hooks for PaelsBlood {
    type Me = RelicId;

    fn modify_hand_draw(&self, _s: &CombatState, _me: RelicId, count: Decimal) -> Decimal {
        // `player != Owner`: the only player is the owner.
        count + CARDS
    }
}
