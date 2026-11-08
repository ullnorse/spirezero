//! `MiniatureCannon` (`Models.Relics/MiniatureCannon.cs`): upgraded cards deal 3 more powered
//! attack damage.

use crate::combat::hooks::{DamageQuery, Hooks};
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DynamicVar("ExtraDamage", 3m)`.
const EXTRA_DAMAGE: i32 = 3;

pub struct MiniatureCannon;

impl RelicModel for MiniatureCannon {
    fn name(&self) -> &'static str {
        "Miniature Cannon"
    }
}

#[sts2_macros::hooks]
impl Hooks for MiniatureCannon {
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
        if !query.props.is_powered_attack() || !s.card(card).is_upgraded() {
            return Decimal::ZERO;
        }
        Decimal::from(EXTRA_DAMAGE)
    }
}
