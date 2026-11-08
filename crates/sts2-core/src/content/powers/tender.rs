//! `TenderPower` (`Models.Powers/TenderPower.cs`): a debuff (`Counter`) on the player. Each card
//! the player plays takes 1 Strength and 1 Dexterity away until the turn ends, when the total
//! comes back (`DisplayAmount` is the number of cards played this turn).

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Tender;

/// `TenderPower`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_cardsPlayedThisTurn`.
    pub cards_played_this_turn: i32,
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Tender(data) => data,
        _ => unreachable!("a Tender instance holds Tender data"),
    }
}

fn set_cards_played(eng: &mut Engine<'_>, me: PowerRef, value: i32) {
    eng.s.set_power_data(
        me.id,
        PowerData::Tender(Data {
            cards_played_this_turn: value,
        }),
    );
}

impl PowerModel for Tender {
    fn name(&self) -> &'static str {
        "Tender"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Tender(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Tender {
    type Me = PowerRef;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, _play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner == Owner.Player`: every card belongs to the player.
        if eng.s.player.creature != me.owner {
            return Ok(());
        }
        let played = data(eng.s, me).cards_played_this_turn + 1;
        set_cards_played(eng, me, played);
        let applier = eng.s.power(me.owner, me.id).applier;
        let minus_one = Decimal::from(-1);
        eng.apply_power(me.owner, PowerKind::Strength, minus_one, applier)?;
        eng.apply_power(me.owner, PowerKind::Dexterity, minus_one, applier)
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        let played = data(eng.s, me).cards_played_this_turn;
        let applier = eng.s.power(me.owner, me.id).applier;
        eng.apply_power(
            me.owner,
            PowerKind::Strength,
            Decimal::from(played),
            applier,
        )?;
        eng.apply_power(
            me.owner,
            PowerKind::Dexterity,
            Decimal::from(played),
            applier,
        )?;
        set_cards_played(eng, me, 0);
        Ok(())
    }
}
