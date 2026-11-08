//! `OneTwoPunchPower` (`Models.Powers/OneTwoPunchPower.cs`): a buff (`Counter`); the owner's next
//! `Amount` Attacks are played one extra time. Removed at the end of the owner's side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::cards::CardType;
use crate::content::powers::{PowerModel, PowerType};

pub struct OneTwoPunch;

impl PowerModel for OneTwoPunch {
    fn name(&self) -> &'static str {
        "One-Two Punch"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for OneTwoPunch {
    type Me = PowerRef;

    fn modify_card_play_count(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _target: Option<CreatureId>,
        count: u32,
    ) -> u32 {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if s.player.creature != me.owner || s.card(card).card_type() != CardType::Attack {
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
}
