//! `DuplicationPower` (`Models.Powers/DuplicationPower.cs`): the owner's next card is played one
//! extra time (one stack per play); removed at the end of its side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerModel, PowerType};

pub struct Duplication;

impl PowerModel for Duplication {
    fn name(&self) -> &'static str {
        "Duplication"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Duplication {
    type Me = PowerRef;

    // Every card belongs to the only player.
    fn modify_card_play_count(
        &self,
        s: &CombatState,
        me: PowerRef,
        _card: CardId,
        _target: Option<CreatureId>,
        count: u32,
    ) -> u32 {
        if s.player.creature != me.owner {
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
