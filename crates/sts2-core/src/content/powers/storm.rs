//! `StormPower` (`Models.Powers/StormPower.cs`): a buff (`Counter`); each Power card the owner
//! plays channels the power's amount of Lightning once it is played. The amount is the one the
//! power had when the card started playing (so the Storm that applied it does not trigger it,
//! and playing Storm again does not channel extra), kept in the instance data between
//! `BeforeCardPlayed` and `AfterCardPlayed`.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::CardType;
use crate::content::powers::{PowerData, PowerModel, PowerType};

pub struct Storm;

/// Cards that can be mid-play at once (nested auto-plays).
const SLOTS: usize = 8;

/// `StormPower.Data`: the `amountsForPlayedCards` dictionary, a fixed array because `PowerData`
/// is `Copy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub amounts_for_played_cards: [Option<(CardId, i32)>; SLOTS],
}

fn data(eng: &Engine<'_>, me: PowerRef) -> Data {
    match *eng.s.power_data(me.id) {
        PowerData::Storm(data) => data,
        _ => unreachable!("a Storm instance holds Storm data"),
    }
}

impl PowerModel for Storm {
    fn name(&self) -> &'static str {
        "Storm"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Storm(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Storm {
    type Me = PowerRef;

    // `cardPlay.Card.Owner != Owner.Player` never holds: the only player owns every card.
    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if eng.s.card(play.card).card_type() != CardType::Power {
            return Ok(());
        }
        let mut data = data(eng, me);
        let amount = eng.s.power(me.owner, me.id).amount;
        let slot = data
            .amounts_for_played_cards
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("more nested card plays than Storm tracks");
        *slot = Some((play.card, amount));
        eng.s.set_power_data(me.id, PowerData::Storm(data));
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        let mut data = data(eng, me);
        let Some(slot) = data
            .amounts_for_played_cards
            .iter_mut()
            .find(|slot| slot.is_some_and(|(card, _)| card == play.card))
        else {
            return Ok(());
        };
        let (_, lightning) = slot.take().expect("the slot was found occupied");
        eng.s.set_power_data(me.id, PowerData::Storm(data));
        for _ in 0..lightning.max(0) {
            eng.channel(OrbKind::Lightning)?;
        }
        Ok(())
    }
}
