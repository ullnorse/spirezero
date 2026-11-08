//! `SpikedGauntlets` (`Models.Relics/SpikedGauntlets.cs`): Ancient (Tanx); +1 max energy, and
//! Power cards cost 1 more (`TryModifyEnergyCostInCombat`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, RelicId};
use crate::content::cards::CardType;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;

pub struct SpikedGauntlets;

impl RelicModel for SpikedGauntlets {
    fn name(&self) -> &'static str {
        "Spiked Gauntlets"
    }
}

#[sts2_macros::hooks]
impl Hooks for SpikedGauntlets {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        amount + ENERGY
    }

    // Every card belongs to the only player.
    fn try_modify_energy_cost_in_combat(
        &self,
        s: &CombatState,
        _me: RelicId,
        card: CardId,
        cost: Decimal,
    ) -> Option<Decimal> {
        (s.card(card).card_type() == CardType::Power).then(|| cost + Decimal::ONE)
    }
}
