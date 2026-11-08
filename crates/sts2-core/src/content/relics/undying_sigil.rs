//! `UndyingSigil` (`Models.Relics/UndyingSigil.cs`): enemies that Doom will kill deal half damage
//! to the owner. (The class comment about Doom checking for the relic is stale: `DoomPower`
//! does not read it.)

use crate::combat::hooks::{DamageQuery, Hooks};
use crate::combat::{CombatState, RelicId};
use crate::content::powers::PowerKind;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `DynamicVar("DamageDecrease", 0.5m)`.
const DAMAGE_DECREASE: Decimal = Decimal::new(5, 1);

pub struct UndyingSigil;

impl RelicModel for UndyingSigil {
    fn name(&self) -> &'static str {
        "Undying Sigil"
    }
}

#[sts2_macros::hooks]
impl Hooks for UndyingSigil {
    type Me = RelicId;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        _me: RelicId,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        let owner = s.player.creature;
        let Some(dealer) = query.dealer else {
            return Decimal::ONE;
        };
        if !query.props.is_powered_attack() || query.target != Some(owner) || dealer == owner {
            return Decimal::ONE;
        }
        let dealer = s.creature(dealer);
        if dealer.hp > dealer.power_amount(PowerKind::Doom) {
            return Decimal::ONE;
        }
        DAMAGE_DECREASE
    }
}
