//! `DanseMacabrePower` (`Models.Powers/DanseMacabrePower.cs`): before the owner plays a card
//! costing at least 2 (`EnergyVar(2)`), gain `Amount` unpowered Block.

use crate::combat::ValueProps;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, Engine, Flow, Hooked};
use crate::content::powers::{PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct DanseMacabre;

/// `DynamicVars.Energy`.
const ENERGY: i32 = 2;

impl PowerModel for DanseMacabre {
    fn name(&self) -> &'static str {
        "Danse Macabre"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for DanseMacabre {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner.Creature == Owner`: every card belongs to the player.
        if eng.s.player.creature != me.owner
            || resolved_energy_cost(eng.hooked(), play.card) < ENERGY
        {
            return Ok(());
        }
        let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
        eng.gain_block(me.owner, amount, ValueProps::UNPOWERED, None)?;
        Ok(())
    }
}

/// `CardEnergyCost.GetResolved`: the captured X for an X cost, else the cost with all modifiers.
fn resolved_energy_cost(h: Hooked<'_>, card: CardId) -> i32 {
    let cost = &h.s.card(card).energy_cost;
    if cost.costs_x {
        cost.captured_x
    } else {
        h.energy_cost(card)
    }
}
