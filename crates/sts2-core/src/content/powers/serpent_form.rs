//! `SerpentFormPower` (`Models.Powers/SerpentFormPower.cs`): a buff (`Counter`); after each card
//! the owner plays, `Amount` unpowered damage to a random hittable enemy. The amount is the one
//! the power had when the card started playing (so the card that applied it does not trigger it),
//! kept in the instance data between `BeforeCardPlayed` and `AfterCardPlayed`.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct SerpentForm;

/// Cards that can be mid-play at once (nested auto-plays).
const SLOTS: usize = 8;

/// `SerpentFormPower.Data`: the `amountsForPlayedCards` dictionary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub amounts_for_played_cards: [Option<(CardId, i32)>; SLOTS],
}

fn data(eng: &Engine<'_>, me: PowerRef) -> Data {
    match *eng.s.power_data(me.id) {
        PowerData::SerpentForm(data) => data,
        _ => unreachable!("a SerpentForm instance holds SerpentForm data"),
    }
}

impl PowerModel for SerpentForm {
    fn name(&self) -> &'static str {
        "Serpent Form"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::SerpentForm(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for SerpentForm {
    type Me = PowerRef;

    // `cardPlay.Card.Owner != Owner.Player` never holds: the only player owns every card.
    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        let mut data = data(eng, me);
        let amount = eng.s.power(me.owner, me.id).amount;
        let slot = data
            .amounts_for_played_cards
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("more nested card plays than SerpentForm tracks");
        *slot = Some((play.card, amount));
        eng.s.set_power_data(me.id, PowerData::SerpentForm(data));
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
        let (_, damage) = slot.take().expect("the slot was found occupied");
        eng.s.set_power_data(me.id, PowerData::SerpentForm(data));
        if damage > 0 {
            let hittable = eng.hooked().hittable_enemies();
            if let Some(target) = eng.random_target(&hittable) {
                eng.damage(
                    &[target],
                    Decimal::from(damage),
                    ValueProps::UNPOWERED,
                    Some(me.owner),
                    None,
                )?;
            }
        }
        Ok(())
    }
}
