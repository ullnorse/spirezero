//! `OblivionPower` (`Models.Powers/OblivionPower.cs`): a debuff that remembers its amount when
//! the applier's player plays a card; after that card, the owner gets that much Doom. Removed at
//! the end of the player side's turn.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerInstanceType, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Oblivion;

/// Cards played at once (nested through `AutoPlay`) that one instance can track.
const SLOTS: usize = 8;

/// `OblivionPower.Data`: `amountsForPlayedCards`, a dictionary from the card to the amount when
/// it was played, as a fixed array because `PowerData` is `Copy`. The in-flight plays nest, so a
/// handful of entries is the most it can hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub amounts_for_played_cards: [Option<(CardId, i32)>; SLOTS],
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Oblivion(data) => data,
        _ => unreachable!("an Oblivion instance holds Oblivion data"),
    }
}

impl PowerModel for Oblivion {
    fn name(&self) -> &'static str {
        "Oblivion"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::InstancedPerApplier
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Oblivion(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Oblivion {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `Applier?.Player == null` / `cardPlay.Card.Owner != Applier.Player`: the applier must
        // be the player, who owns every card.
        if eng.s.power(me.owner, me.id).applier != Some(eng.s.player.creature) {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        let slots = &mut data_mut(eng.s, me).amounts_for_played_cards;
        // `Dictionary.Add` throws on a duplicate key.
        assert!(
            slots.iter().flatten().all(|&(card, _)| card != play.card),
            "Oblivion: a card is played twice at once"
        );
        let slot = slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("Oblivion tracks at most SLOTS nested plays");
        *slot = Some((play.card, amount));
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        let slots = &mut data_mut(eng.s, me).amounts_for_played_cards;
        let Some(slot) = slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|(card, _)| card == play.card))
        else {
            return Ok(());
        };
        let (_, amount) = slot.take().expect("the slot was found by its card");
        let applier = eng.s.power(me.owner, me.id).applier;
        eng.apply_power(me.owner, PowerKind::Doom, Decimal::from(amount), applier)
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        side: Side,
        _participants: &[CreatureId],
    ) -> Flow {
        if side == Side::Player {
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
