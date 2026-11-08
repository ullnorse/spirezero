//! `CuriousPower` (`Models.Powers/CuriousPower.cs`): the owner's Power cards cost `Amount` less
//! (not below 0, and a free Power stays free). `MadScience`'s Curious rider applies it.

use crate::combat::CardId;
use crate::combat::CombatState;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Curious;

impl PowerModel for Curious {
    fn name(&self) -> &'static str {
        "Curious"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Curious {
    type Me = PowerRef;

    /// `TryModifyEnergyCostInCombat`: `card.Owner.Creature != Owner` never holds (one player).
    fn try_modify_energy_cost_in_combat(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        original: Decimal,
    ) -> Option<Decimal> {
        if s.card(card).card_type() != CardType::Power || original <= Decimal::ZERO {
            return None;
        }
        let modified = original - Decimal::from(s.power(me.owner, me.id).amount);
        Some(modified.max(Decimal::ZERO))
    }
}
