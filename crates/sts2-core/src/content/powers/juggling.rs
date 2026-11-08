//! `JugglingPower` (`Models.Powers/JugglingPower.cs`): a buff (`Counter`); when the owner starts
//! its third Attack of a turn, `Amount` clones of that Attack are added to the hand. The count
//! starts from the Attacks already started this turn and resets at the end of the owner's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{
    CardPlay, CombatState, CreatureId, Engine, Flow, HistoryKind, HistoryTag, PileType, Side,
};
use crate::content::cards::CardType;
use crate::content::powers::{PowerData, PowerModel, PowerType};

pub struct Juggling;

/// `JugglingPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub attacks_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> Option<&mut Data> {
    s.creature(me.owner).powers.iter().find(|p| p.id == me.id)?;
    match s.power_data_mut(me.id) {
        PowerData::Juggling(data) => Some(data),
        _ => unreachable!("a Juggling instance holds Juggling data"),
    }
}

impl PowerModel for Juggling {
    fn name(&self) -> &'static str {
        "Juggling"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Juggling(Data::default())
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        // `CardPlaysStarted` of the owner's Attacks this turn.
        let started = eng
            .s
            .history_this_turn(HistoryTag::CardPlayStarted)
            .iter()
            .filter(|e| match &e.kind {
                HistoryKind::CardPlayStarted(play) => {
                    eng.s.card(play.card).card_type() == CardType::Attack
                }
                _ => false,
            })
            .count();
        let started = i32::try_from(started).expect("a count fits i32");
        if let Some(data) = data_mut(eng.s, me) {
            data.attacks_played_this_turn = started;
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Juggling {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner != Owner.Player`: every card belongs to the only player.
        if eng.s.player.creature != me.owner
            || eng.s.card(play.card).card_type() != CardType::Attack
        {
            return Ok(());
        }
        let Some(data) = data_mut(eng.s, me) else {
            return Ok(());
        };
        data.attacks_played_this_turn += 1;
        if data.attacks_played_this_turn != 3 {
            return Ok(());
        }
        // `Flash` is visual only.
        for _ in 0..eng.s.power(me.owner, me.id).amount {
            let card = eng.create_clone(play.card);
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(me.owner))?;
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
            data.attacks_played_this_turn = 0;
        }
        Ok(())
    }
}
