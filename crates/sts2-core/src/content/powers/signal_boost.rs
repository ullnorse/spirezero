//! `SignalBoostPower` (`Models.Powers/SignalBoostPower.cs`): a buff (`Counter`); the owner's next
//! `Amount` Power cards are played one extra time (the decomp tests `CardType.Power`, not Skill).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};

pub struct SignalBoost;

impl PowerModel for SignalBoost {
    fn name(&self) -> &'static str {
        "Signal Boost"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for SignalBoost {
    type Me = PowerRef;

    // Every card belongs to the only player.
    fn modify_card_play_count(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _target: Option<CreatureId>,
        count: u32,
    ) -> u32 {
        if s.player.creature != me.owner || s.card(card).card_type() != CardType::Power {
            return count;
        }
        count + 1
    }

    fn after_modifying_card_play_count(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
    ) -> Flow {
        eng.decrement_power(me)
    }
}
