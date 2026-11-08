//! `IterationPower` (`Models.Powers/IterationPower.cs`): a buff (`Counter`); the first Status
//! card the owner draws each turn draws `Amount` more cards.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, Engine, Flow, HistoryKind, HistoryTag};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Iteration;

impl PowerModel for Iteration {
    fn name(&self) -> &'static str {
        "Iteration"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Iteration {
    type Me = PowerRef;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        // `card.Owner.Creature == Owner`: every card belongs to the player.
        if eng.s.player.creature != me.owner || eng.s.card(card).card_type() != CardType::Status {
            return Ok(());
        }
        let drawn = eng
            .s
            .history_this_turn(HistoryTag::CardDrawn)
            .iter()
            .filter(|e| {
                e.actor == me.owner
                    && matches!(&e.kind, HistoryKind::CardDrawn { card, .. }
                        if eng.s.card(*card).card_type() == CardType::Status)
            })
            .count();
        if drawn <= 1 {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.draw(amount, false)?;
        }
        Ok(())
    }
}
