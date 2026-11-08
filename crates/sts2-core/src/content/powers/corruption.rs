//! `CorruptionPower` (`Models.Powers/CorruptionPower.cs`): a buff (`Single`); the owner's Skills
//! cost 0 and are exhausted when played.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, Resources, ResultLocation, ResultPile};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Corruption;

impl PowerModel for Corruption {
    fn name(&self) -> &'static str {
        "Corruption"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Corruption {
    type Me = PowerRef;

    fn try_modify_energy_cost_in_combat_late(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _cost: Decimal,
    ) -> Option<Decimal> {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        (s.player.creature == me.owner && s.card(card).card_type() == CardType::Skill)
            .then_some(Decimal::ZERO)
    }

    // `AfterModifyingCardPlayResultLocation` only flashes.
    fn modify_card_play_result_location(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _is_auto_play: bool,
        _resources: Resources,
        location: ResultLocation,
    ) -> ResultLocation {
        if s.player.creature != me.owner || s.card(card).card_type() != CardType::Skill {
            return location;
        }
        ResultLocation {
            pile: ResultPile::Exhaust,
            ..location
        }
    }
}
