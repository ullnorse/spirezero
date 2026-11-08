//! `StrikeDummy` (`Models.Relics/StrikeDummy.cs`): cards tagged Strike (`Tags`, the canonical tags)
//! deal 3 more powered attack damage.

use crate::combat::hooks::{DamageQuery, Hooks};
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DynamicVar("ExtraDamage", 3m)`.
const EXTRA_DAMAGE: i32 = 3;

pub struct StrikeDummy;

impl RelicModel for StrikeDummy {
    fn name(&self) -> &'static str {
        "Strike Dummy"
    }
}

#[sts2_macros::hooks]
impl Hooks for StrikeDummy {
    type Me = RelicId;

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        _me: RelicId,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        // `dealer != owner && cardSource.Owner != owner` vetoes; the only player owns every card.
        let Some(card) = query.card else {
            return Decimal::ZERO;
        };
        if !query.props.is_powered_attack() || !s.card(card).kind.is_strike() {
            return Decimal::ZERO;
        }
        Decimal::from(EXTRA_DAMAGE)
    }
}
