//! `FeralPower` (`Models.Powers/FeralPower.cs`): a buff (`Counter`); the owner's first `Amount`
//! Attacks played for 0 energy each turn return to the top of the hand instead of their result
//! pile. The count starts from those already played this turn and resets at the start of the
//! owner's turn.

use crate::combat::commands::Position;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{
    CardId, CombatState, CreatureId, Engine, Flow, HistoryKind, HistoryTag, Resources,
    ResultLocation, ResultPile, Side,
};
use crate::content::cards::CardType;
use crate::content::powers::{PowerData, PowerModel, PowerType};

pub struct Feral;

/// `FeralPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub zero_cost_attacks_played: i32,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Feral(data) => data,
        _ => unreachable!("a Feral instance holds Feral data"),
    }
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> Option<&mut Data> {
    s.creature(me.owner).powers.iter().find(|p| p.id == me.id)?;
    match s.power_data_mut(me.id) {
        PowerData::Feral(data) => Some(data),
        _ => unreachable!("a Feral instance holds Feral data"),
    }
}

impl PowerModel for Feral {
    fn name(&self) -> &'static str {
        "Feral"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Feral(Data::default())
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        // `CardPlayStartedEntry`s this turn of the owner's Attacks with `EnergyValue == 0`.
        let started = eng
            .s
            .history_this_turn(HistoryTag::CardPlayStarted)
            .iter()
            .filter(|e| match &e.kind {
                HistoryKind::CardPlayStarted(play) => {
                    eng.s.card(play.card).card_type() == CardType::Attack && play.energy_value == 0
                }
                _ => false,
            })
            .count();
        let started = i32::try_from(started).expect("a count fits i32");
        if let Some(data) = data_mut(eng.s, me) {
            data.zero_cost_attacks_played = started;
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Feral {
    type Me = PowerRef;

    fn modify_card_play_result_location(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _is_auto_play: bool,
        resources: Resources,
        location: ResultLocation,
    ) -> ResultLocation {
        // `card.Owner.Creature != Owner`: every card belongs to the player.
        if s.player.creature != me.owner
            || s.card(card).card_type() != CardType::Attack
            || resources.energy > 0
            || s.card(card).is_dupe
            || data(s, me).zero_cost_attacks_played >= s.power(me.owner, me.id).amount
        {
            return location;
        }
        ResultLocation {
            pile: ResultPile::Hand,
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
        // Flash is visual only.
        if let Some(data) = data_mut(eng.s, me) {
            data.zero_cost_attacks_played += 1;
        }
        Ok(())
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner)
            && let Some(data) = data_mut(eng.s, me)
        {
            data.zero_cost_attacks_played = 0;
        }
        Ok(())
    }
}
