//! `StranglePower` (`Models.Powers/StranglePower.cs`): a debuff (`Counter`, `InstancedPerApplier`).
//! Remembers its amount when the applier's player starts a card; after that card, the owner takes
//! that much unblockable, unpowered damage. Removed at the end of its owner's side's turn.
//!
//! The player is the only applier in scope, so one instance per target is equivalent to the
//! per-applier instances.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::powers::{PowerData, PowerInstanceType, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Strangle;

/// Cards played at once (nested through `AutoPlay`) that one instance can track.
const SLOTS: usize = 8;

/// `StranglePower.Data`: `amountsForPlayedCards`, a dictionary from the card to the amount when
/// it was played, as a fixed array because `PowerData` is `Copy`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub amounts_for_played_cards: [Option<(CardId, i32)>; SLOTS],
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Strangle(data) => data,
        _ => unreachable!("a Strangle instance holds Strangle data"),
    }
}

impl PowerModel for Strangle {
    fn name(&self) -> &'static str {
        "Strangle"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn instance_type(&self) -> PowerInstanceType {
        PowerInstanceType::InstancedPerApplier
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Strangle(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Strangle {
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
            "Strangle: a card is played twice at once"
        );
        let slot = slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("Strangle tracks at most SLOTS nested plays");
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
        eng.damage(
            &[me.owner],
            Decimal::from(amount),
            ValueProps::UNBLOCKABLE.with(ValueProps::UNPOWERED),
            None,
            None,
        )?;
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
            eng.remove_power(me.owner, me.id)?;
        }
        Ok(())
    }
}
