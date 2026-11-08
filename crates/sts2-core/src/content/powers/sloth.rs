//! `SlothPower` (`Models.Powers/SlothPower.cs`): a debuff (`Counter`) from the `KnowledgeDemon`'s
//! Curse of Knowledge: its owner's player can play at most `Amount` cards per turn
//! (`ShouldPlay`, manual and auto-plays). `_cardsPlayedThisTurn` counts each card play started
//! (`BeforeCardPlayed`) and resets when the owner's side starts a turn.

use crate::combat::AutoPlayType;
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerModel, PowerType};

/// At most `Amount` cards per turn.
pub struct Sloth;

/// The instance's fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_cardsPlayedThisTurn` (also the `DisplayAmount`).
    pub cards_played_this_turn: i32,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Sloth(data) => data,
        _ => unreachable!("a Sloth instance holds Sloth data"),
    }
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Sloth(data) => data,
        _ => unreachable!("a Sloth instance holds Sloth data"),
    }
}

impl PowerModel for Sloth {
    fn name(&self) -> &'static str {
        "Sloth"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Sloth(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Sloth {
    type Me = PowerRef;

    /// `card.Owner.Creature != Owner`: every card is the player's.
    fn should_play(
        &self,
        s: &CombatState,
        me: PowerRef,
        _card: CardId,
        _auto_play: AutoPlayType,
    ) -> bool {
        if me.owner != s.player.creature {
            return true;
        }
        data(s, me).cards_played_this_turn < s.power(me.owner, me.id).amount
    }

    /// `cardPlay.Card.Owner != Owner.Player`: a pet owner has no `Player`.
    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, _play: &CardPlay) -> Flow {
        if eng.s.creature(me.owner).is_player() {
            data_mut(eng.s, me).cards_played_this_turn += 1;
        }
        Ok(())
    }

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&me.owner) {
            data_mut(eng.s, me).cards_played_this_turn = 0;
        }
        Ok(())
    }
}
