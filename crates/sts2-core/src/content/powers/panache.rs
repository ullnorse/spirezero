//! `PanachePower` (`Models.Powers/PanachePower.cs`): an `Instanced` buff (`Counter`). Every 5th
//! card the owner plays (not counting the play that applied this instance) deals `Amount`
//! unpowered damage to every hittable enemy; the count restarts at the end of the owner's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerData, PowerInstanceType, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Panache;

/// `CardsLeft` (`DynamicVar("CardsLeft", 5m)`, reset to `_baseCardsLeft`).
const BASE_CARDS_LEFT: i32 = 5;

/// `PanachePower.Data` plus the instance's `CardsLeft` var.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    /// `DynamicVars["CardsLeft"]`.
    pub cards_left: i32,
    /// `alreadyApplied`: the play that applied this instance has been seen.
    pub already_applied: bool,
}

/// The instance's data, if it is still on its owner.
fn data_mut(s: &mut CombatState, me: PowerRef) -> Option<&mut Data> {
    s.creature(me.owner).powers.iter().find(|p| p.id == me.id)?;
    match s.power_data_mut(me.id) {
        PowerData::Panache(data) => Some(data),
        _ => unreachable!("a Panache instance holds Panache data"),
    }
}

impl PowerModel for Panache {
    fn name(&self) -> &'static str {
        "Panache"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::Instanced
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Panache(Data {
            cards_left: BASE_CARDS_LEFT,
            already_applied: false,
        })
    }
}

#[sts2_macros::hooks]
impl Hooks for Panache {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, _play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner != Owner.Player`: every card belongs to the player.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let Some(data) = data_mut(eng.s, me) else {
            return Ok(());
        };
        if data.already_applied {
            data.cards_left -= 1;
            if data.cards_left <= 0 {
                let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
                let targets = eng.hooked().hittable_enemies();
                eng.damage(
                    &targets,
                    amount,
                    ValueProps::UNPOWERED,
                    Some(me.owner),
                    None,
                )?;
                if let Some(data) = data_mut(eng.s, me) {
                    data.cards_left = BASE_CARDS_LEFT;
                }
            }
        }
        if let Some(data) = data_mut(eng.s, me) {
            data.already_applied = true;
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
        if participants.contains(&me.owner)
            && let Some(data) = data_mut(eng.s, me)
        {
            data.cards_left = BASE_CARDS_LEFT;
        }
        Ok(())
    }
}
