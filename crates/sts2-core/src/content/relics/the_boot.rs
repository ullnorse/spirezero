//! `TheBoot` (`Models.Relics/TheBoot.cs`): unblocked HP lost by an enemy to a powered attack of
//! the owner (or its Osty) is raised to at least 5 (`DamageMinimum`) when it is at least 1
//! (`ModifyHpLostAfterOstyLate`). `DamageThreshold` (4) is only displayed.

use crate::combat::hooks::{Hooks, HpLossQuery};
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DynamicVar("DamageMinimum", 5m)`.
const DAMAGE_MINIMUM: i32 = 5;

pub struct TheBoot;

impl RelicModel for TheBoot {
    fn name(&self) -> &'static str {
        "The Boot"
    }
}

#[sts2_macros::hooks]
impl Hooks for TheBoot {
    type Me = RelicId;

    // `AfterModifyingHpLostAfterOsty` only flashes.
    fn modify_hp_lost_after_osty_late(
        &self,
        s: &CombatState,
        _me: RelicId,
        query: HpLossQuery,
        amount: Decimal,
    ) -> Decimal {
        // `dealer != Owner.Creature && dealer != Owner.Osty` (a null dealer equals a null Osty).
        let from_owner = query.dealer == Some(s.player.creature) || query.dealer == s.osty();
        let minimum = Decimal::from(DAMAGE_MINIMUM);
        if !from_owner
            || query.target == s.player.creature
            || !query.props.is_powered_attack()
            || amount < Decimal::ONE
            || amount >= minimum
        {
            return amount;
        }
        minimum
    }
}
