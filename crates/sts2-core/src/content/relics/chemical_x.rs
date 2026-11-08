//! `ChemicalX` (`Models.Relics/ChemicalX.cs`): +2 to the X of the owner's X-cost cards.
//! `BeforeCardPlayed` only flashes.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, RelicId};
use crate::content::relics::RelicModel;

/// `DynamicVar("Increase", 2m)` (`IntValue`).
const INCREASE: i32 = 2;

pub struct ChemicalX;

impl RelicModel for ChemicalX {
    fn name(&self) -> &'static str {
        "Chemical X"
    }
}

#[sts2_macros::hooks]
impl Hooks for ChemicalX {
    type Me = RelicId;

    fn modify_x_value(&self, _s: &CombatState, _me: RelicId, _card: CardId, value: i32) -> i32 {
        // The only player is the owner.
        value + INCREASE
    }
}
