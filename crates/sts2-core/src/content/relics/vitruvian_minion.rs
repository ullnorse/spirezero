//! `VitruvianMinion` (`Models.Relics/VitruvianMinion.cs`): damage and block from the owner's
//! Minion cards is doubled.

use crate::combat::CombatState;
use crate::combat::RelicId;
use crate::combat::hooks::{BlockQuery, DamageQuery, Hooks};
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

pub struct VitruvianMinion;

impl RelicModel for VitruvianMinion {
    fn name(&self) -> &'static str {
        "Vitruvian Minion"
    }
}

/// `cardSource != null && cardSource.Owner == Owner && cardSource.Tags.Contains(Minion)`: 2, else
/// 1. Every card belongs to the player.
fn factor(s: &CombatState, card: Option<crate::combat::CardId>) -> Decimal {
    match card {
        Some(card) if s.card(card).kind.is_minion() => Decimal::from(2),
        _ => Decimal::ONE,
    }
}

#[sts2_macros::hooks]
impl Hooks for VitruvianMinion {
    type Me = RelicId;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        _me: RelicId,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        factor(s, query.card)
    }

    fn modify_block_multiplicative(
        &self,
        s: &CombatState,
        _me: RelicId,
        query: BlockQuery,
        _amount: Decimal,
    ) -> Decimal {
        factor(s, query.card)
    }
}
