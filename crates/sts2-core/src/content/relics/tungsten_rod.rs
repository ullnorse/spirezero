//! `TungstenRod` (`Models.Relics/TungstenRod.cs`): the owner loses 1 less HP from every loss.

use crate::combat::hooks::{Hooks, HpLossQuery};
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;
use crate::run::RunState;

/// `DynamicVar("HpLossReduction", 1m)`.
const HP_LOSS_REDUCTION: i32 = 1;

pub struct TungstenRod;

impl RelicModel for TungstenRod {
    fn name(&self) -> &'static str {
        "Tungsten Rod"
    }

    fn modify_hp_lost_out_of_combat(&self, _s: &RunState, _me: usize, amount: Decimal) -> Decimal {
        Decimal::ZERO.max(amount - Decimal::from(HP_LOSS_REDUCTION))
    }
}

#[sts2_macros::hooks]
impl Hooks for TungstenRod {
    type Me = RelicId;

    fn modify_hp_lost_after_osty(
        &self,
        s: &CombatState,
        _me: RelicId,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        if query.target != s.player.creature {
            return amount;
        }
        Decimal::ZERO.max(amount - Decimal::from(HP_LOSS_REDUCTION))
    }
}
