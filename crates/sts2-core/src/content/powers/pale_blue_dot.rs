//! `PaleBlueDotPower` (`Models.Powers/PaleBlueDotPower.cs`): a buff (`Counter`); once per turn,
//! after the owner's 5th card play finished, apply `Amount` `DrawCardsNextTurnPower` to the
//! owner. Reset at the end of the owner's side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, CreatureId, Engine, Flow, HistoryTag, Side};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct PaleBlueDot;

/// `cardPlayThresholdValue`.
const CARD_PLAY_THRESHOLD: usize = 5;

/// `PaleBlueDotPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub already_activated_this_turn: bool,
}

fn set_activated(eng: &mut Engine<'_>, me: PowerRef, activated: bool) {
    eng.s.set_power_data(
        me.id,
        PowerData::PaleBlueDot(Data {
            already_activated_this_turn: activated,
        }),
    );
}

impl PowerModel for PaleBlueDot {
    fn name(&self) -> &'static str {
        "Pale Blue Dot"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::PaleBlueDot(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for PaleBlueDot {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, _play: &CardPlay) -> Flow {
        // The only player owns every card.
        let PowerData::PaleBlueDot(data) = *eng.s.power_data(me.id) else {
            unreachable!("a PaleBlueDot instance holds PaleBlueDot data");
        };
        if data.already_activated_this_turn {
            return Ok(());
        }
        // `AttacksPlayedThisTurn`: the owner's finished card plays this turn.
        let player = eng.s.player.creature;
        let plays = eng
            .s
            .history_this_turn(HistoryTag::CardPlayFinished)
            .iter()
            .filter(|e| e.actor == player)
            .count();
        if plays >= CARD_PLAY_THRESHOLD {
            set_activated(eng, me, true);
            let amount = Decimal::from(eng.s.power(me.owner, me.id).amount);
            eng.apply_power(
                me.owner,
                PowerKind::DrawCardsNextTurn,
                amount,
                Some(me.owner),
            )?;
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
        if participants.contains(&me.owner) {
            set_activated(eng, me, false);
        }
        Ok(())
    }
}
