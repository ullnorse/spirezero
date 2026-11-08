//! `ReboundPower` (`Models.Powers/ReboundPower.cs`): a counter buff; the owner's next `Amount`
//! cards that would go to the discard pile after being played go on top of the draw pile instead
//! (`ModifyCardPlayResultLocation`), each use decrementing it; it is removed at the end of the
//! owner's side's turn.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{
    CardId, CombatState, CreatureId, Engine, Flow, Resources, ResultLocation, ResultPile, Side,
};
use crate::content::powers::{PowerModel, PowerType};

pub struct Rebound;

impl PowerModel for Rebound {
    fn name(&self) -> &'static str {
        "Rebound"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Rebound {
    type Me = PowerRef;

    fn modify_card_play_result_location(
        &self,
        s: &CombatState,
        me: PowerRef,
        _card: CardId,
        _is_auto_play: bool,
        _resources: Resources,
        location: ResultLocation,
    ) -> ResultLocation {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if s.player.creature != me.owner || location.pile != ResultPile::Discard {
            return location;
        }
        ResultLocation {
            pile: ResultPile::Draw,
            position: Position::Top,
        }
    }

    fn after_modifying_card_play_result_location(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _card: CardId,
        _location: ResultLocation,
    ) -> Flow {
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        // Flash: visual only.
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
