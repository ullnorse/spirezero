//! `IceCream` (`Models.Relics/IceCream.cs`): energy is not reset at the start of the owner's
//! turns after the first, so unspent energy carries over.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;

pub struct IceCream;

impl RelicModel for IceCream {
    fn name(&self) -> &'static str {
        "Ice Cream"
    }
}

#[sts2_macros::hooks]
impl Hooks for IceCream {
    type Me = RelicId;

    fn should_player_reset_energy(&self, s: &CombatState, _me: RelicId) -> bool {
        // The only player is the owner.
        s.player.turn_number == 1
    }
}
