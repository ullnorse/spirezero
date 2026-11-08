//! `AfterimagePower` (`Models.Powers/AfterimagePower.cs`): every card the owner plays gives
//! Block (unpowered) equal to the power's `Amount` when the play began. The amounts are kept per
//! card between `BeforeCardPlayed` and `AfterCardPlayed`, so the play that applied the power
//! gives none and a stack applied mid-play does not count for that play.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Afterimage;

/// The most plays that can be in progress at once (a play nests only through auto-plays).
const MAX_PLAYS: usize = 16;

/// `AfterimagePower.Data.amountsForPlayedCards`: card plays between their two hooks, with the
/// amount at the time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Data {
    pub plays: [(CardId, i32); MAX_PLAYS],
    pub len: usize,
}

impl Data {
    const EMPTY: Self = Self {
        plays: [(CardId(0), 0); MAX_PLAYS],
        len: 0,
    };

    /// `Dictionary.Add`.
    fn add(&mut self, card: CardId, amount: i32) {
        assert!(self.len < MAX_PLAYS, "too many nested card plays");
        self.plays[self.len] = (card, amount);
        self.len += 1;
    }

    /// `Dictionary.Remove(card, out value)`.
    fn remove(&mut self, card: CardId) -> Option<i32> {
        let index = self.plays[..self.len]
            .iter()
            .position(|&(c, _)| c == card)?;
        let (_, amount) = self.plays[index];
        self.plays.copy_within(index + 1..self.len, index);
        self.len -= 1;
        Some(amount)
    }
}

impl PowerModel for Afterimage {
    fn name(&self) -> &'static str {
        "Afterimage"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Afterimage(Data::EMPTY)
    }
}

#[sts2_macros::hooks]
impl Hooks for Afterimage {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner.Creature != Owner`: every card is the only player's.
        if me.owner != eng.s.player.creature {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        let PowerData::Afterimage(data) = eng.s.power_data_mut(me.id) else {
            unreachable!("an Afterimage instance holds Afterimage data");
        };
        data.add(play.card, amount);
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if me.owner != eng.s.player.creature {
            return Ok(());
        }
        let PowerData::Afterimage(data) = eng.s.power_data_mut(me.id) else {
            unreachable!("an Afterimage instance holds Afterimage data");
        };
        if let Some(amount) = data.remove(play.card)
            && amount > 0
        {
            eng.gain_block(me.owner, Decimal::from(amount), ValueProps::UNPOWERED, None)?;
        }
        Ok(())
    }
}
