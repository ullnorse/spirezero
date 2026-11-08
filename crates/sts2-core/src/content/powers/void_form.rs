//! `VoidFormPower` (`Models.Powers/VoidFormPower.cs`): a buff (`Counter`); the first `Amount`
//! cards its owner plays by hand each turn cost no energy and no stars while in the Hand or Play
//! pile (`TryModifyEnergyCostInCombatLate`, `TryModifyStarCost`).
//!
//! `HideTemporaryZeroCostVisual` sets the count to 999999999 in `BeforeApplied` and when the
//! power's amount is about to change, so the cards stay paid until the owner's next turn starts.
//! `BeforeApplied` runs on the new instance before it is added, so a new instance starts there
//! ([`PowerModel::init_internal_data`]).

use crate::combat::hooks::{Hooks, PowerQuery, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, PileType, Side};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct VoidForm;

/// `HideTemporaryZeroCostVisual`'s count.
const HIDDEN: i32 = 999_999_999;

/// `VoidFormPower.Data`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub cards_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::VoidForm(data) => data,
        _ => unreachable!("a VoidForm instance holds VoidForm data"),
    }
}

/// `ShouldSkip(card)`: the card is not the owner's, not in the Hand or Play pile, or the
/// owner already played `Amount` cards this turn.
fn should_skip(s: &CombatState, me: PowerRef, card: CardId) -> bool {
    if me.owner != s.player.creature
        || !matches!(s.pile_of(card), Some(PileType::Hand | PileType::Play))
    {
        return true;
    }
    let PowerData::VoidForm(data) = *s.power_data(me.id) else {
        unreachable!("a VoidForm instance holds VoidForm data");
    };
    data.cards_played_this_turn >= s.power(me.owner, me.id).amount
}

impl PowerModel for VoidForm {
    fn name(&self) -> &'static str {
        "Void Form"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        // `new Data()`, then `BeforeApplied` hides the zero costs.
        PowerData::VoidForm(Data {
            cards_played_this_turn: HIDDEN,
        })
    }
}

#[sts2_macros::hooks]
impl Hooks for VoidForm {
    type Me = PowerRef;

    fn before_power_amount_changed(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        power: PowerQuery,
        _amount: Decimal,
    ) -> Flow {
        // `power == this`: an existing instance only changes by stacking onto it, and a
        // creature holds one VoidForm.
        if power.kind == PowerKind::VoidForm && power.target == me.owner {
            data_mut(eng.s, me).cards_played_this_turn = HIDDEN;
        }
        Ok(())
    }

    fn try_modify_energy_cost_in_combat_late(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _cost: Decimal,
    ) -> Option<Decimal> {
        (!should_skip(s, me, card)).then_some(Decimal::ZERO)
    }

    fn try_modify_star_cost(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _cost: Decimal,
    ) -> Option<Decimal> {
        (!should_skip(s, me, card)).then_some(Decimal::ZERO)
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner.Creature == Owner`: the only player's cards.
        if me.owner == eng.s.player.creature && !play.is_auto_play && play.is_last_in_series() {
            let data = data_mut(eng.s, me);
            data.cards_played_this_turn = data.cards_played_this_turn.saturating_add(1);
        }
        Ok(())
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            data_mut(eng.s, me).cards_played_this_turn = 0;
        }
        Ok(())
    }
}
