//! `Bread` (`Models.Relics/Bread.cs`): 2 energy fewer on the first turn, 1 more max energy on
//! every later turn.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar("GainEnergy", 1)`.
const GAIN_ENERGY: Decimal = Decimal::ONE;
/// `EnergyVar("LoseEnergy", 2)`.
const LOSE_ENERGY: Decimal = Decimal::new(2, 0);

pub struct Bread;

impl RelicModel for Bread {
    fn name(&self) -> &'static str {
        "Bread"
    }
}

#[sts2_macros::hooks]
impl Hooks for Bread {
    type Me = RelicId;

    fn modify_max_energy(&self, s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        // The only player is the owner, and it has a combat state during combat.
        if s.player.turn_number == 1 {
            return amount;
        }
        amount + GAIN_ENERGY
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number == 1 {
            eng.lose_energy(LOSE_ENERGY);
        }
        Ok(())
    }
}
