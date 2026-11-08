//! `AutomationPower` (`Models.Powers/AutomationPower.cs`): an `Instanced` buff (`Counter`). Every
//! 10th card the owner draws gains `Amount` energy.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, Engine, Flow};
use crate::content::powers::{PowerData, PowerInstanceType, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Automation;

/// `_baseCardsLeft`.
const BASE_CARDS_LEFT: i32 = 10;

/// `AutomationPower.Data`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `cardsLeft`.
    pub cards_left: i32,
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Automation(data) => data,
        _ => unreachable!("an Automation instance holds Automation data"),
    }
}

impl PowerModel for Automation {
    fn name(&self) -> &'static str {
        "Automation"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Automation(Data {
            cards_left: BASE_CARDS_LEFT,
        })
    }
}

#[sts2_macros::hooks]
impl Hooks for Automation {
    type Me = PowerRef;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        // `card.Owner == Owner.Player`: every card belongs to the player.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.cards_left -= 1;
        if data.cards_left <= 0 {
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.gain_energy(amount)?;
            if eng
                .s
                .creature(me.owner)
                .powers
                .iter()
                .any(|p| p.id == me.id)
            {
                data_mut(eng.s, me).cards_left = BASE_CARDS_LEFT;
            }
        }
        Ok(())
    }
}
