//! `SneckoSkull` (`Models.Relics/SneckoSkull.cs`): the owner's Poison applications are 1 higher.

use crate::combat::hooks::{Hooks, PowerQuery};
use crate::combat::{CombatState, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `PowerVar<PoisonPower>(1m)`.
const POISON: i32 = 1;

pub struct SneckoSkull;

impl RelicModel for SneckoSkull {
    fn name(&self) -> &'static str {
        "Snecko Skull"
    }
}

#[sts2_macros::hooks]
impl Hooks for SneckoSkull {
    type Me = RelicId;

    fn modify_power_amount_given_additive(
        &self,
        s: &CombatState,
        _me: RelicId,
        power: PowerQuery,
        _amount: Decimal,
    ) -> Decimal {
        if power.kind != PowerKind::Poison || power.applier != Some(s.player.creature) {
            return Decimal::ZERO;
        }
        Decimal::from(POISON)
    }
}
