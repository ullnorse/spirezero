//! `TangledPower` (`Models.Powers/TangledPower.cs`): a debuff (`Counter`) that afflicts every
//! Attack of its owner's player with `Entangled`, now and as they enter combat, and makes each
//! Entangled card cost `Amount` more. Removed at the end of its owner's turn, which clears every
//! `Entangled`.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, PileType, Side};
use crate::content::afflictions::AfflictionKind;
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

/// `VineShambler`'s debuff on Attack costs.
pub struct Tangled;

/// `PlayerCombatState.AllCards`: the cards of every combat pile, in pile order.
pub fn all_cards(s: &CombatState) -> Vec<CardId> {
    PileType::LISTENER_ORDER
        .into_iter()
        .flat_map(|p| s.player.pile(p).iter().copied())
        .collect()
}

fn is_entangled(s: &CombatState, card: CardId) -> bool {
    s.card(card)
        .affliction
        .is_some_and(|a| a.kind == AfflictionKind::Entangled)
}

impl PowerModel for Tangled {
    fn name(&self) -> &'static str {
        "Tangled"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        _me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        for card in all_cards(eng.s) {
            if eng.s.card(card).card_type() == CardType::Attack {
                eng.afflict(card, AfflictionKind::Entangled, Decimal::ONE);
            }
        }
        Ok(())
    }

    fn after_removed(&self, eng: &mut Engine<'_>, _power: &Power, _old_owner: CreatureId) -> Flow {
        for card in all_cards(eng.s) {
            if is_entangled(eng.s, card) {
                eng.clear_affliction(card);
            }
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Tangled {
    type Me = PowerRef;

    /// The owner is the only player, so every combat card is its player's.
    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: PowerRef, card: CardId) -> Flow {
        let c = eng.s.card(card);
        if eng.s.creature(me.owner).is_player()
            && c.affliction.is_none()
            && c.card_type() == CardType::Attack
        {
            eng.afflict(card, AfflictionKind::Entangled, Decimal::ONE);
        }
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }

    fn try_modify_energy_cost_in_combat(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        cost: Decimal,
    ) -> Option<Decimal> {
        if !is_entangled(s, card) || !s.creature(me.owner).is_player() {
            return None;
        }
        Some(cost + Decimal::from(s.power(me.owner, me.id).amount))
    }
}
