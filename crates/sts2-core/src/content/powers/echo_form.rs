//! `EchoFormPower` (`Models.Powers/EchoFormPower.cs`): a buff (`Counter`); the first `Amount`
//! card plays of each turn that start a series get one extra play (`ModifyCardPlayCount`). The
//! flash and the vfx are presentation.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CombatState, CreatureId, HistoryKind, HistoryTag};
use crate::content::powers::{PowerModel, PowerType};

pub struct EchoForm;

impl PowerModel for EchoForm {
    fn name(&self) -> &'static str {
        "Echo Form"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for EchoForm {
    type Me = PowerRef;

    fn modify_card_play_count(
        &self,
        s: &CombatState,
        me: PowerRef,
        _card: CardId,
        _target: Option<CreatureId>,
        count: u32,
    ) -> u32 {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if s.player.creature != me.owner {
            return count;
        }
        let started = s
            .history_this_turn(HistoryTag::CardPlayStarted)
            .iter()
            .filter(|e| {
                e.actor == me.owner
                    && matches!(&e.kind, HistoryKind::CardPlayStarted(play) if play.is_first_in_series())
            })
            .count();
        let amount = usize::try_from(s.power(me.owner, me.id).amount).unwrap_or(0);
        if started >= amount {
            return count;
        }
        count + 1
    }
}
