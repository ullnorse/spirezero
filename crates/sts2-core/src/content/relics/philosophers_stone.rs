//! `PhilosophersStone` (`Models.Relics/PhilosophersStone.cs`): +1 max energy; every enemy gains
//! 1 Strength when the combat room is entered, and so does every creature that joins the
//! enemies' side later.

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: Decimal = Decimal::ONE;

pub struct PhilosophersStone;

impl RelicModel for PhilosophersStone {
    fn name(&self) -> &'static str {
        "Philosopher's Stone"
    }
}

#[sts2_macros::hooks]
impl Hooks for PhilosophersStone {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        amount + ENERGY
    }

    fn after_creature_added_to_combat(
        &self,
        eng: &mut Engine<'_>,
        _me: RelicId,
        creature: CreatureId,
    ) -> Flow {
        let owner = eng.s.player.creature;
        if eng.s.creature(creature).side == eng.s.creature(owner).side {
            return Ok(());
        }
        eng.apply_power(creature, PowerKind::Strength, STRENGTH, None)?;
        Ok(())
    }

    fn after_room_entered(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // Every combat's room is a `CombatRoom`.
        // `GetOpponentsOf(Owner.Creature)`: the enemy side.
        let targets: Vec<CreatureId> = eng
            .s
            .creatures_on(Side::Enemy)
            .into_iter()
            .filter(|&c| eng.s.creature(c).is_alive())
            .collect();
        eng.apply_power_to_all(&targets, PowerKind::Strength, STRENGTH, None)?;
        Ok(())
    }
}
