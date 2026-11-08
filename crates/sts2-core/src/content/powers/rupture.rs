//! `RupturePower` (`Models.Powers/RupturePower.cs`): when the owner loses HP on its side's turn
//! it gains `Amount` Strength. HP lost to a card the owner is playing is tallied per card
//! (`playedCards`) and applied once that card has been played.

use crate::combat::hooks::{DamageEvent, Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Rupture;

/// How many cards can be in play at once (nested auto-plays); `playedCards` is a dictionary.
const SLOTS: usize = 8;

/// `RupturePower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `playedCards`: each card being played and the Strength owed for it.
    pub played_cards: [Option<(CardId, i32)>; SLOTS],
}

impl Data {
    fn find_mut(&mut self, card: CardId) -> Option<&mut i32> {
        self.played_cards
            .iter_mut()
            .flatten()
            .find_map(|(c, owed)| (*c == card).then_some(owed))
    }
}

fn data_mut<'a>(eng: &'a mut Engine<'_>, me: PowerRef) -> &'a mut Data {
    let PowerData::Rupture(data) = eng.s.power_data_mut(me.id) else {
        unreachable!("a Rupture instance holds Rupture data");
    };
    data
}

impl PowerModel for Rupture {
    fn name(&self) -> &'static str {
        "Rupture"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Rupture(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Rupture {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner.Creature != Owner`: every card belongs to the player.
        if eng.s.player.creature != me.owner || eng.s.side != eng.s.creature(me.owner).side {
            return Ok(());
        }
        let data = data_mut(eng, me);
        // `Dictionary.Add`.
        let slot = data
            .played_cards
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("fewer than eight cards are in play at once");
        *slot = Some((play.card, 0));
        Ok(())
    }

    fn after_damage_received(&self, eng: &mut Engine<'_>, me: PowerRef, hit: DamageEvent) -> Flow {
        if hit.target != me.owner
            || hit.result.unblocked <= 0
            || eng.s.side != eng.s.creature(me.owner).side
        {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        let in_play = hit
            .card
            .and_then(|card| data_mut(eng, me).find_mut(card).map(|owed| *owed += amount));
        if in_play.is_none() {
            eng.apply_power(
                me.owner,
                PowerKind::Strength,
                Decimal::from(amount),
                Some(me.owner),
            )?;
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let data = data_mut(eng, me);
        let removed = data
            .played_cards
            .iter_mut()
            .find(|slot| slot.is_some_and(|(card, _)| card == play.card))
            .and_then(Option::take);
        if let Some((_, owed)) = removed {
            eng.apply_power(
                me.owner,
                PowerKind::Strength,
                Decimal::from(owed),
                Some(me.owner),
            )?;
        }
        Ok(())
    }
}
