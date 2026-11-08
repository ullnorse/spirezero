//! `MysticLighter` (`Models.Relics/MysticLighter.cs`): powered attacks from an enchanted card
//! deal 9 more damage.

use crate::combat::hooks::{DamageQuery, Hooks};
use crate::combat::{CombatState, RelicId};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DamageVar(9m, ValueProp.Unpowered)` (`IntValue`).
const DAMAGE: i32 = 9;

pub struct MysticLighter;

impl RelicModel for MysticLighter {
    fn name(&self) -> &'static str {
        "Mystic Lighter"
    }
}

#[sts2_macros::hooks]
impl Hooks for MysticLighter {
    type Me = RelicId;

    fn modify_damage_additive(
        &self,
        s: &CombatState,
        _me: RelicId,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        // `cardSource.Owner != Owner` vetoes; the only player owns every card.
        let Some(card) = query.card else {
            return Decimal::ZERO;
        };
        if !query.props.is_powered_attack() || s.card(card).enchantment.is_none() {
            return Decimal::ZERO;
        }
        Decimal::from(DAMAGE)
    }
}
